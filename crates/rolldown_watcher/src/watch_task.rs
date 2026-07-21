use arcstr::ArcStr;
use rolldown::{Bundler, BundlerBuilder, BundlerConfig};
use rolldown_common::{
  BundleMode, LogLevel, NormalizedBundlerOptions, ScanMode, WatcherChangeKind,
};
use rolldown_error::{
  BatchedBuildDiagnostic, BuildDiagnostic, BuildResult, Diagnostic, DiagnosticOptions, ResultExt,
  filter_out_disabled_diagnostics,
};
use rolldown_fs_watcher::{DynFsWatcher, RecursiveMode};
use rolldown_utils::{dashmap::FxDashSet, pattern_filter};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Mutex as TokioMutex;

use crate::event::{BundleEndEventData, BundleStartEventData, WatchErrorEventData};

oxc_index::define_index_type! {
  pub struct WatchTaskIdx = u32;
}

/// Per-task data container that owns a bundler and its file-system watcher.
pub struct WatchTask {
  bundler: Arc<TokioMutex<Bundler>>,
  options: Arc<NormalizedBundlerOptions>,
  fs_watcher: std::sync::Mutex<DynFsWatcher>,
  watched_files: FxDashSet<ArcStr>,
  pub(crate) needs_rebuild: bool,
  closed: Arc<AtomicBool>,
}

impl WatchTask {
  pub(crate) fn new(
    config: BundlerConfig,
    fs_watcher: DynFsWatcher,
    closed: &Arc<AtomicBool>,
  ) -> BuildResult<Self> {
    // Validation: dev_mode not allowed with watch
    if config.options.experimental.as_ref().and_then(|e| e.dev_mode.as_ref()).is_some() {
      return Err(
        BuildDiagnostic::bundler_initialize_error(
          "The \"experimental.devMode\" option is only supported with the \"dev\" API. \
           It cannot be used with \"watch\". Please use the \"dev\" API for dev mode functionality."
            .to_string(),
          None,
        )
        .into(),
      );
    }

    let bundler = BundlerBuilder::default()
      .with_options(config.options)
      .with_plugins(config.plugins)
      .build()?;

    let options = Arc::clone(bundler.options());

    Ok(Self {
      bundler: Arc::new(TokioMutex::new(bundler)),
      options,
      fs_watcher: std::sync::Mutex::new(fs_watcher),
      watched_files: FxDashSet::default(),
      needs_rebuild: true,
      closed: Arc::clone(closed),
    })
  }

  /// Run a build and return the outcome for the caller to emit events.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(crate) async fn build(&mut self, task_index: WatchTaskIdx) -> BuildResult<BuildOutcome> {
    if !self.needs_rebuild {
      return Ok(BuildOutcome::Skipped);
    }

    let start_time = Instant::now();

    let skip_write = self.options.watch.skip_write;
    // Use field-level borrows so the closure can capture fs_watcher/watched_files/options
    // without conflicting with the &mut self borrow on bundler.
    let fs_watcher_ref = &self.fs_watcher;
    let watched_files_ref = &self.watched_files;
    let options_ref = &*self.options;

    // Scope the bundler lock to minimize lock duration
    let closed = Arc::clone(&self.closed);
    let (result, new_watch_files, bundle_handle) = {
      let mut bundler = self.bundler.lock().await;

      // Clear stale plugin driver state from previous build.
      if let Some(last_bundle_handle) = &bundler.last_bundle_handle {
        last_bundle_handle.plugin_driver().clear();
      }

      // Always clear the resolver and tsconfig caches before each rebuild in
      // watch mode to avoid stale resolution and transform results from
      // modified package.json/tsconfig/export maps.
      bundler.clear_resolver_cache();
      bundler.clear_transform_tsconfig_cache();

      // Use with_cached_bundle_experimental to register FS watches between scan and write phases.
      // This ensures changes made during render hooks (e.g. renderStart modifying a file)
      // are detected by the FS watcher.
      let result = bundler
        .with_cached_bundle_experimental(BundleMode::FullBuild, async |bundle| {
          let scan_result = bundle.scan_modules(ScanMode::Full).await;

          // Register watch files discovered during scan BEFORE checking scan errors
          // (so files are watched even on error — enables recovery when user fixes the issue)
          let watch_files: Vec<ArcStr> =
            bundle.get_watch_files().iter().map(|f| f.clone()).collect();
          Self::update_watch_files_from(
            fs_watcher_ref,
            watched_files_ref,
            options_ref,
            &watch_files,
          )?;

          let scan_output = scan_result?;

          // Watcher closed mid-build: signal cancellation to the caller via `None`.
          if closed.load(Ordering::Relaxed) {
            return Ok(None);
          }

          let output = if skip_write {
            bundle.bundle_generate(scan_output).await?
          } else {
            bundle.bundle_write(scan_output).await?
          };
          Ok(Some(output))
        })
        .await;

      // Extract the bundle handle for event data (watch files + close support)
      let bundle_handle =
        bundler.last_bundle_handle.clone().expect("bundle handle should exist after build");

      // Collect watch files while we have the lock (may include render-phase files)
      let new_watch_files: Vec<ArcStr> =
        bundle_handle.watch_files().iter().map(|f| f.clone()).collect();

      (result, new_watch_files, bundle_handle)
    };

    // Also register any files discovered during render/write phase
    self.update_watch_files(&new_watch_files)?;

    #[expect(clippy::cast_possible_truncation)]
    let duration = start_time.elapsed().as_millis() as u32;

    self.needs_rebuild = false;

    match result {
      Ok(None) => Ok(BuildOutcome::Closed),
      Ok(Some(output)) => {
        // Emit build warnings (e.g. CIRCULAR_DEPENDENCY) via the on_log callback,
        // matching the behavior of the non-watch build path.
        if let Err(err) = Self::emit_warnings(&self.options, output.warnings).await {
          return Ok(BuildOutcome::Error(WatchErrorEventData {
            task_index,
            diagnostics: Arc::from(BatchedBuildDiagnostic::from(err).into_vec()),
            cwd: self.options.cwd.clone(),
            bundle_handle,
          }));
        }
        Ok(BuildOutcome::Success(BundleEndEventData {
          task_index,
          output: resolve_output_path(
            &self.options.cwd,
            self.options.file.as_deref().unwrap_or(&self.options.out_dir),
          )
          .to_string_lossy()
          .into_owned(),
          duration,
          bundle_handle,
        }))
      }
      Err(errs) => Ok(BuildOutcome::Error(WatchErrorEventData {
        task_index,
        diagnostics: Arc::from(errs.into_vec()),
        cwd: self.options.cwd.clone(),
        bundle_handle,
      })),
    }
  }

  /// Emit build warnings via the `on_log` callback.
  /// This mirrors the warning-handling logic in the non-watch build path so that
  /// diagnostics such as CIRCULAR_DEPENDENCY are surfaced during watch rebuilds.
  async fn emit_warnings(
    options: &NormalizedBundlerOptions,
    warnings: Vec<BuildDiagnostic>,
  ) -> anyhow::Result<()> {
    if warnings.is_empty() || options.log_level == Some(LogLevel::Silent) {
      return Ok(());
    }
    let Some(on_log) = options.on_log.as_ref() else {
      return Ok(());
    };

    let warnings: Vec<BuildDiagnostic> =
      filter_out_disabled_diagnostics(warnings, &options.checks).collect();
    if warnings.is_empty() {
      return Ok(());
    }

    // Render all warnings through the batch API so the per-source line index /
    // ariadne `Source` is built once and shared, rather than rebuilt for every
    // warning (O(N^2) for many warnings in one large file, see #9748).
    let diagnostic_options = DiagnosticOptions { cwd: options.cwd.clone() };
    let diagnostics: Vec<Diagnostic> =
      warnings.iter().map(|warning| warning.to_diagnostic_with(&diagnostic_options)).collect();
    let rendered = Diagnostic::render_batch(&diagnostics, true);

    // Dispatch sequentially, awaiting each callback before the next, so a handler
    // that throws to abort the build stops at the first failure without invoking
    // later handlers. Mirrors `handle_warnings` in the binding. See #9748.
    for (warning, rendered) in warnings.into_iter().zip(rendered) {
      #[expect(
        clippy::cast_possible_truncation,
        reason = "line/column/position values are unlikely to exceed u32::MAX in practical use"
      )]
      let (loc, pos) = match rendered.primary_location {
        Some(location) => (
          Some(rolldown_common::LogLocation {
            line: location.line as u32,
            column: location.column as u32,
            file: warning.id(),
          }),
          Some(location.utf16_position as u32),
        ),
        None => (None, None),
      };
      on_log
        .call(
          LogLevel::Warn,
          rolldown_common::Log {
            id: warning.id(),
            exporter: warning.exporter(),
            code: Some(warning.kind().to_string()),
            message: rendered.message,
            plugin: None,
            loc,
            pos,
            ids: warning.ids(),
          },
        )
        .await?;
    }
    Ok(())
  }

  /// Start event data for this task
  pub(crate) fn start_event_data(&self, task_index: WatchTaskIdx) -> BundleStartEventData {
    BundleStartEventData { task_index }
  }

  /// Update watched files by adding new ones to the fs watcher.
  fn update_watch_files(&self, files: &[ArcStr]) -> BuildResult<()> {
    Self::update_watch_files_from(&self.fs_watcher, &self.watched_files, &self.options, files)
  }

  /// Static helper: update FS watcher with newly discovered files.
  /// Separated from `&self` to allow calling from closures during build.
  fn update_watch_files_from(
    fs_watcher: &std::sync::Mutex<DynFsWatcher>,
    watched_files: &FxDashSet<ArcStr>,
    options: &NormalizedBundlerOptions,
    files: &[ArcStr],
  ) -> BuildResult<()> {
    let mut fs_watcher = fs_watcher.lock().expect("fs_watcher lock poisoned");
    let mut watcher_paths = fs_watcher.paths_mut();

    for file in files {
      let file_str = file.as_str();
      if watched_files.contains(file_str) {
        continue;
      }
      let path = Path::new(file_str);
      if !path.exists() {
        continue;
      }
      if pattern_filter::filter(
        options.watch.exclude.as_deref(),
        options.watch.include.as_deref(),
        file_str,
        options.cwd.to_string_lossy().as_ref(),
      )
      .inner()
      {
        match watcher_paths.add(path, RecursiveMode::NonRecursive) {
          Ok(()) => {
            tracing::debug!(name = "notify watch", path = ?path);
            watched_files.insert(file.clone());
          }
          Err(e) => {
            tracing::debug!(name = "notify watch skipped", path = ?path, error = ?e);
          }
        }
      }
    }

    watcher_paths.commit().map_err_to_unhandleable()?;

    Ok(())
  }

  /// Mark this task as needing rebuild if the changed file is in our watch list.
  /// Returns `true` if the file is relevant to this task.
  pub(crate) fn mark_needs_rebuild(&mut self, path: &str) -> bool {
    if self.is_watched_file(path) {
      self.needs_rebuild = true;
      return true;
    }
    false
  }

  /// Call on_invalidate callback if the path is in watch list
  pub(crate) async fn call_on_invalidate(&self, path: &str) {
    if self.is_watched_file(path) {
      let bundler = self.bundler.lock().await;
      if let Some(on_invalidate) = &bundler.options().watch.on_invalidate {
        on_invalidate.call(path);
      }
    }
  }

  /// Call watch_change plugin hook
  #[tracing::instrument(level = "debug", skip(self))]
  pub(crate) async fn call_watch_change(&self, path: &str, kind: WatcherChangeKind) {
    let bundler = self.bundler.lock().await;
    if let Some(plugin_driver) =
      bundler.last_bundle_handle.as_ref().map(rolldown::BundleHandle::plugin_driver)
    {
      let _ = plugin_driver.watch_change(path, kind).await.map_err(|e| {
        tracing::error!("watch_change plugin hook error: {e:?}");
      });
    }
  }

  /// Call close_watcher plugin hooks
  #[tracing::instrument(level = "debug", skip_all)]
  pub(crate) async fn call_hook_close_watcher(&self) {
    let bundler = self.bundler.lock().await;
    if let Some(last_bundle_handle) = &bundler.last_bundle_handle {
      let _ = last_bundle_handle.plugin_driver().close_watcher().await.map_err(|e| {
        tracing::error!("close_watcher plugin hook error: {e:?}");
      });
    }
  }

  /// Close the bundler (calls `closeBundle` plugin hook for the last built bundle, if any).
  #[tracing::instrument(level = "debug", skip_all)]
  pub(crate) async fn close(&self) -> anyhow::Result<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.close().await.map_err(Into::into)
  }

  fn is_watched_file(&self, path: &str) -> bool {
    if self.watched_files.contains(path) {
      return true;
    }

    // Windows path normalization
    #[cfg(windows)]
    if self.watched_files.contains(path.replace('\\', "/").as_str()) {
      return true;
    }

    false
  }
}

/// Resolve the output path reported by `BundleEnd` events.
///
/// Rollup reports the resolved `output.file` (falling back to `output.dir`)
/// here, so honor `file` when it is set instead of always using `out_dir`,
/// and normalize `.`/`..` components for a stable absolute path.
///
/// See "API Contract" in `internal-docs/watch-mode/implementation.md`.
fn resolve_output_path(cwd: &Path, output: &str) -> PathBuf {
  let output = Path::new(output);
  let path = join_absolute(cwd, output);
  let absolute_path = if path.is_absolute() {
    path
  } else {
    std::env::current_dir().map_or(path.clone(), |current_dir| join_absolute(&current_dir, &path))
  };

  let mut normalized = PathBuf::new();
  for component in absolute_path.components() {
    match component {
      Component::CurDir => {}
      Component::ParentDir => {
        normalized.pop();
      }
      Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
        normalized.push(component.as_os_str());
      }
    }
  }
  normalized
}

/// Join `path` onto `base`, producing an absolute path when `base` is absolute.
///
/// This is `Path::join` plus handling for Windows drive-relative paths such as
/// `C:foo` (a `Prefix` component without a `RootDir`). Plain `join` would let
/// such a path replace `base` entirely (`PathBuf::push`: "if `path` has a
/// prefix but no root, it replaces `self`"), leaking a non-absolute path.
fn join_absolute(base: &Path, path: &Path) -> PathBuf {
  if path.is_absolute() {
    return path.to_path_buf();
  }

  #[cfg(windows)]
  if let Some(Component::Prefix(prefix)) = path.components().next()
    && let std::path::Prefix::Disk(drive) = prefix.kind()
    && !path.has_root()
  {
    let remainder: PathBuf = path.components().skip(1).collect();
    let base_drive = match base.components().next() {
      Some(Component::Prefix(base_prefix)) => match base_prefix.kind() {
        std::path::Prefix::Disk(d) | std::path::Prefix::VerbatimDisk(d) => Some(d),
        _ => None,
      },
      _ => None,
    };
    return if drive_relative_uses_base(drive, base_drive) {
      base.join(remainder)
    } else {
      // `base` lives on another drive (or has no disk prefix), so it cannot
      // anchor the path; per-drive current directories are process state we do
      // not track, so fall back to the drive's root.
      let mut resolved = PathBuf::from(format!("{}:\\", char::from(drive)));
      resolved.push(remainder);
      resolved
    };
  }

  base.join(path)
}

/// Decision logic for a Windows drive-relative path (e.g. `C:foo`): it may be
/// resolved against the base directory only when both sit on the same drive
/// (drive letters compare ASCII case-insensitively); otherwise it must be
/// resolved from its own drive.
#[cfg(any(windows, test))]
fn drive_relative_uses_base(path_drive: u8, base_drive: Option<u8>) -> bool {
  base_drive.is_some_and(|base_drive| base_drive.eq_ignore_ascii_case(&path_drive))
}

/// Outcome of a build attempt
pub enum BuildOutcome {
  /// Build was skipped (no rebuild needed)
  Skipped,
  /// Build succeeded
  Success(BundleEndEventData),
  /// Build had errors (but didn't fail fatally)
  Error(WatchErrorEventData),
  /// `watcher.close()` was called during the build; output was discarded.
  Closed,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn relative_output_path_is_resolved_and_normalized() {
    let cwd = std::env::current_dir().expect("current directory");
    assert_eq!(resolve_output_path(&cwd, "./nested/../dist/out.js"), cwd.join("dist/out.js"));

    let absolute = cwd.join("absolute.js");
    assert_eq!(resolve_output_path(&cwd, absolute.to_string_lossy().as_ref()), absolute);
  }

  #[test]
  fn drive_relative_decision_logic() {
    // Same drive (ASCII case-insensitive): resolve against the base directory.
    assert!(drive_relative_uses_base(b'C', Some(b'C')));
    assert!(drive_relative_uses_base(b'c', Some(b'C')));
    assert!(drive_relative_uses_base(b'D', Some(b'd')));
    // Different drive: the base directory cannot anchor the path.
    assert!(!drive_relative_uses_base(b'D', Some(b'C')));
    // Base without a disk prefix (e.g. a UNC path): same.
    assert!(!drive_relative_uses_base(b'C', None));
  }

  /// `output.file` is arbitrary user config, so drive-relative (`C:bundle.js`)
  /// and root-relative (`\bundle.js`) forms must still resolve to the absolute
  /// path promised by the `BUNDLE_END.output` contract.
  #[cfg(windows)]
  #[test]
  fn windows_drive_relative_output_path_resolves_against_cwd() {
    let cwd = PathBuf::from(r"C:\proj");
    // Drive-relative on the same drive resolves inside `cwd`.
    assert_eq!(resolve_output_path(&cwd, "C:bundle.js"), PathBuf::from(r"C:\proj\bundle.js"));
    // Drive letters match case-insensitively; the prefix comes from `cwd`.
    assert_eq!(resolve_output_path(&cwd, "c:bundle.js"), PathBuf::from(r"C:\proj\bundle.js"));
    // Drive-relative on another drive resolves from that drive's root.
    assert_eq!(resolve_output_path(&cwd, "D:bundle.js"), PathBuf::from(r"D:\bundle.js"));
    // Root-relative keeps `cwd`'s drive.
    assert_eq!(resolve_output_path(&cwd, r"\bundle.js"), PathBuf::from(r"C:\bundle.js"));
  }
}
