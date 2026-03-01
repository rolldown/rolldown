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
    let options_ref = &*self.options;

    // Scope the bundler lock to minimize lock duration
    let (result, new_watch_files) = {
      let mut bundler = self.bundler.lock().await;

      // Reset bundler state for watch mode — allows rebuilds after `result.close()`
      // (which sets closed=true) and clears stale plugin driver state.
      bundler.reset_closed_for_watch_mode();
      if let Some(last_bundle_handle) = &bundler.last_bundle_handle {
        last_bundle_handle.plugin_driver().clear();
      }

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

          if skip_write {
            bundle.bundle_generate(scan_output).await
          } else {
            bundle.bundle_write(scan_output).await
          }
        })
        .await;

      // Collect watch files while we have the lock (may include render-phase files)
      let new_watch_files: Vec<ArcStr> = bundler.watch_files().iter().map(|f| f.clone()).collect();

      (result, new_watch_files)
    };

    // Also register any files discovered during render/write phase
    self.update_watch_files(&new_watch_files)?;

    #[expect(clippy::cast_possible_truncation)]
    let duration = start_time.elapsed().as_millis() as u32;

    self.needs_rebuild = false;

    match result {
      Ok(_output) => Ok(BuildOutcome::Success(BundleEndEventData {
        task_index,
        output: self.options.cwd.join(&self.options.out_dir).to_string_lossy().into_owned(),
        duration,
        bundler: Arc::clone(&self.bundler),
      })),
      Err(errs) => Ok(BuildOutcome::Error(WatchErrorEventData {
        task_index,
        diagnostics: Arc::from(errs.into_vec()),
        cwd: self.options.cwd.clone(),
        bundler: Arc::clone(&self.bundler),
      })),
    }
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
      if path.exists()
        && pattern_filter::filter(
          options.watch.exclude.as_deref(),
          options.watch.include.as_deref(),
          file_str,
          options.cwd.to_string_lossy().as_ref(),
        )
        .inner()
      {
        tracing::debug!(name = "notify watch", path = ?path);
        watcher_paths.add(path, RecursiveMode::NonRecursive).map_err_to_unhandleable()?;
        watched_files.insert(file.clone());
      }
    }
    watcher_paths.commit().map_err_to_unhandleable()?;

    Ok(())
  }

  /// Mark this task as needing rebuild if the changed file is in our watch list.
  pub(crate) fn mark_needs_rebuild(&mut self, path: &str) {
    if self.is_watched_file(path) {
      self.needs_rebuild = true;
    }
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

  /// Close the bundler
  #[tracing::instrument(level = "debug", skip_all)]
  pub(crate) async fn close(&self) -> anyhow::Result<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.close().await?;
    Ok(())
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
