use arcstr::ArcStr;
use rolldown::{Bundler, BundlerBuilder, BundlerConfig};
use rolldown_common::{BundleMode, NormalizedBundlerOptions, ScanMode, WatcherChangeKind};
use rolldown_error::{BuildDiagnostic, BuildResult, ResultExt};
use rolldown_fs_watcher::{DynFsWatcher, RecursiveMode};
use rolldown_utils::{dashmap::FxDashSet, pattern_filter};
use std::path::Path;
use std::sync::Arc;
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
  /// Directories registered with the fs watcher for missing import detection.
  /// Only used to avoid re-registering the same directory with notify.
  registered_missing_dirs: FxDashSet<ArcStr>,
  /// Active missing import directories from the latest build.
  /// Refreshed each build from `plugin_driver.missing_import_dirs`.
  /// Only directories in this set trigger rebuilds on `Create` events.
  active_missing_dirs: FxDashSet<ArcStr>,
  pub(crate) needs_rebuild: bool,
}

impl WatchTask {
  pub(crate) fn new(config: BundlerConfig, fs_watcher: DynFsWatcher) -> BuildResult<Self> {
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
      registered_missing_dirs: FxDashSet::default(),
      active_missing_dirs: FxDashSet::default(),
      needs_rebuild: true,
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
    let registered_missing_dirs_ref = &self.registered_missing_dirs;
    let active_missing_dirs_ref = &self.active_missing_dirs;
    let options_ref = &*self.options;

    // Clear active missing dirs before rebuilding — will be repopulated from
    // plugin_driver.missing_import_dirs during scan.
    self.active_missing_dirs.clear();

    // Scope the bundler lock to minimize lock duration
    let (result, new_watch_files, bundle_handle) = {
      let mut bundler = self.bundler.lock().await;

      // Clear stale plugin driver state from previous build.
      if let Some(last_bundle_handle) = &bundler.last_bundle_handle {
        last_bundle_handle.plugin_driver().clear();
      }

      // Always clear resolver cache before each rebuild in watch mode to avoid
      // stale resolution results from modified package.json/tsconfig/export maps.
      bundler.clear_resolver_cache();

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
          let new_missing_dirs: Vec<ArcStr> =
            bundle.get_missing_import_dirs().iter().map(|f| f.clone()).collect();
          Self::update_watch_files_from(
            fs_watcher_ref,
            watched_files_ref,
            registered_missing_dirs_ref,
            active_missing_dirs_ref,
            options_ref,
            &watch_files,
            &new_missing_dirs,
          )?;

          let scan_output = scan_result?;

          if skip_write {
            bundle.bundle_generate(scan_output).await
          } else {
            bundle.bundle_write(scan_output).await
          }
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
    // (missing_import_dirs are only populated during scan, so pass empty here)
    self.update_watch_files(&new_watch_files, &[])?;

    #[expect(clippy::cast_possible_truncation)]
    let duration = start_time.elapsed().as_millis() as u32;

    self.needs_rebuild = false;

    match result {
      Ok(_output) => Ok(BuildOutcome::Success(BundleEndEventData {
        task_index,
        output: self.options.cwd.join(&self.options.out_dir).to_string_lossy().into_owned(),
        duration,
        bundle_handle,
      })),
      Err(errs) => Ok(BuildOutcome::Error(WatchErrorEventData {
        task_index,
        diagnostics: Arc::from(errs.into_vec()),
        cwd: self.options.cwd.clone(),
        bundle_handle,
      })),
    }
  }

  /// Start event data for this task
  pub(crate) fn start_event_data(&self, task_index: WatchTaskIdx) -> BundleStartEventData {
    BundleStartEventData { task_index }
  }

  /// Update watched files by adding new ones to the fs watcher.
  fn update_watch_files(&self, files: &[ArcStr], missing_dirs: &[ArcStr]) -> BuildResult<()> {
    Self::update_watch_files_from(
      &self.fs_watcher,
      &self.watched_files,
      &self.registered_missing_dirs,
      &self.active_missing_dirs,
      &self.options,
      files,
      missing_dirs,
    )
  }

  /// Static helper: update FS watcher with newly discovered files.
  /// Separated from `&self` to allow calling from closures during build.
  fn update_watch_files_from(
    fs_watcher: &std::sync::Mutex<DynFsWatcher>,
    watched_files: &FxDashSet<ArcStr>,
    registered_missing_dirs: &FxDashSet<ArcStr>,
    active_missing_dirs: &FxDashSet<ArcStr>,
    options: &NormalizedBundlerOptions,
    files: &[ArcStr],
    new_missing_dirs: &[ArcStr],
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

    // Watch directories where imports failed to resolve, so we can detect
    // when the missing file is created.
    for dir in new_missing_dirs {
      active_missing_dirs.insert(dir.clone());

      if registered_missing_dirs.contains(dir.as_str()) {
        continue;
      }

      // Find the nearest existing ancestor directory to watch. The target
      // directory itself may not exist yet (e.g. `import './new-folder/file.js'`).
      // Only mark as "registered" when we watch the exact target dir — if we
      // fell back to an ancestor, we need to retry on the next build so that
      // once the directory exists, we add a direct watch on it.
      let dir_path = Path::new(dir.as_str());
      let watch_path = std::iter::successors(Some(dir_path), |p| p.parent()).find(|p| p.exists());
      if let Some(watch_path) = watch_path {
        match watcher_paths.add(watch_path, RecursiveMode::NonRecursive) {
          Ok(()) => {
            tracing::debug!(name = "notify watch missing dir", target = ?dir_path, watching = ?watch_path);
            if watch_path == dir_path {
              registered_missing_dirs.insert(dir.clone());
            }
          }
          Err(e) => {
            tracing::debug!(name = "notify watch missing dir skipped", path = ?watch_path, error = ?e);
          }
        }
      }
    }

    watcher_paths.commit().map_err_to_unhandleable()?;

    Ok(())
  }

  /// Mark this task as needing rebuild if the changed file is in our watch list
  /// or (for Create events) if it was created in a directory with missing imports.
  /// Returns `true` if the file is relevant to this task.
  pub(crate) fn mark_needs_rebuild(&mut self, path: &str, kind: WatcherChangeKind) -> bool {
    if self.is_watched_file(path) {
      self.needs_rebuild = true;
      return true;
    }
    // For Create events, check if the file was created in a directory where
    // a relative import failed to resolve in the latest build, or if the
    // created path itself is a missing directory (ancestor fallback case:
    // when the target dir didn't exist, we watched an ancestor and need to
    // detect the directory creation).
    if kind == WatcherChangeKind::Create {
      let event_path = Path::new(path);
      // Check if the created path itself is a tracked missing directory
      if self.active_missing_dirs.contains(event_path.to_string_lossy().as_ref()) {
        self.needs_rebuild = true;
        return true;
      }
      // Check if the file was created inside a tracked missing directory
      if let Some(parent) = event_path.parent() {
        if self.active_missing_dirs.contains(parent.to_string_lossy().as_ref()) {
          self.needs_rebuild = true;
          return true;
        }
      }
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

/// Outcome of a build attempt
pub enum BuildOutcome {
  /// Build was skipped (no rebuild needed)
  Skipped,
  /// Build succeeded
  Success(BundleEndEventData),
  /// Build had errors (but didn't fail fatally)
  Error(WatchErrorEventData),
}
