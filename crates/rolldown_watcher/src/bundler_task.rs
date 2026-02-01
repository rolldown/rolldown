use crate::emitter::SharedWatcherEmitter;
use crate::event::{BundleEndEventData, BundleErrorEventData, BundleEvent, WatcherEvent};
use crate::state::ChangeEntry;
use anyhow::Result;
use arcstr::ArcStr;
use notify::{RecommendedWatcher, WatchMode, Watcher};
use rolldown::{Bundler, BundlerBuilder, BundlerConfig};
use rolldown_common::WatcherChangeKind;
use rolldown_error::{BuildDiagnostic, BuildResult, ResultExt};
use rolldown_utils::{dashmap::FxDashSet, pattern_filter, pattern_filter::StringOrRegex};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Mutex;

/// Per-bundler task that handles building and file watching
pub struct BundlerTask {
  /// Index of this bundler in the watcher's list
  index: usize,
  /// The bundler instance
  bundler: Arc<Mutex<Bundler>>,
  /// Shared event emitter
  emitter: SharedWatcherEmitter,
  /// Shared notify watcher for file system events
  notify_watcher: Arc<Mutex<RecommendedWatcher>>,
  /// Set of files being watched
  watch_files: FxDashSet<ArcStr>,
  /// Flag indicating if this task needs to rebuild
  needs_rebuild: AtomicBool,
  /// Bundler's working directory
  cwd: PathBuf,
  /// Bundler's output path
  output_path: String,
  /// Watch options
  watch_exclude: Option<Vec<StringOrRegex>>,
  watch_include: Option<Vec<StringOrRegex>>,
}

impl BundlerTask {
  /// Create a new bundler task from a config
  pub fn new(
    index: usize,
    config: BundlerConfig,
    emitter: SharedWatcherEmitter,
    notify_watcher: Arc<Mutex<RecommendedWatcher>>,
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

    // Extract options we need before building the bundler
    let cwd = config.options.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
    let out_dir = config.options.dir.clone().unwrap_or_else(|| "dist".to_string());
    let output_path = cwd.join(&out_dir).to_string_lossy().to_string();
    let watch_exclude = config.options.watch.as_ref().and_then(|w| w.exclude.clone());
    let watch_include = config.options.watch.as_ref().and_then(|w| w.include.clone());

    let bundler = BundlerBuilder::default()
      .with_options(config.options)
      .with_plugins(config.plugins)
      .build()?;

    Ok(Self {
      index,
      bundler: Arc::new(Mutex::new(bundler)),
      emitter,
      notify_watcher,
      watch_files: FxDashSet::default(),
      needs_rebuild: AtomicBool::new(true), // Initial build needed
      cwd,
      output_path,
      watch_exclude,
      watch_include,
    })
  }

  /// Check if this bundler should rebuild based on changed files
  #[expect(dead_code, reason = "exposed for potential future use")]
  pub fn should_rebuild(&self, changes: &[ChangeEntry]) -> bool {
    // Always rebuild on initial run (empty changes)
    if changes.is_empty() {
      return self.needs_rebuild.load(Ordering::Relaxed);
    }

    // Check if any changed file is in our watch list
    for change in changes {
      if self.watch_files.contains(change.path.as_str()) {
        return true;
      }

      // Windows path normalization
      #[cfg(windows)]
      if self.watch_files.contains(change.path.replace('\\', "/").as_str()) {
        return true;
      }
    }

    false
  }

  /// Invalidate the bundler, marking it for rebuild
  pub fn invalidate(&self, path: &str) {
    if self.watch_files.contains(path) {
      self.needs_rebuild.store(true, Ordering::Relaxed);
    }

    // Windows path normalization
    #[cfg(windows)]
    if self.watch_files.contains(path.replace('\\', "/").as_str()) {
      self.needs_rebuild.store(true, Ordering::Relaxed);
    }
  }

  /// Handle a file change event
  #[tracing::instrument(level = "debug", skip(self))]
  pub fn on_change(&self, path: &str, kind: WatcherChangeKind) {
    // Mark for rebuild if the file is in our watch list
    // Note: `kind` is currently unused but kept for API compatibility
    let _ = kind;
    self.invalidate(path);
  }

  /// Run a build using the public bundler API
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn build(&self) -> BuildResult<()> {
    if !self.needs_rebuild.load(Ordering::Relaxed) {
      return Ok(());
    }

    let start_time = Instant::now();

    self.emitter.emit(WatcherEvent::Event(BundleEvent::BundleStart { bundler_index: self.index }));

    // Scope the bundler lock to minimize lock duration
    let (result, new_watch_files) = {
      let mut bundler = self.bundler.lock().await;

      // Use the public write() method
      let result = bundler.write().await;

      // Collect watch files while we have the lock
      let new_watch_files: Vec<ArcStr> = bundler.watch_files().iter().map(|f| f.clone()).collect();

      (result, new_watch_files)
    };

    // Now update watch files without holding the bundler lock
    self.update_watch_files(&new_watch_files).await?;

    let duration = {
      #[expect(clippy::cast_possible_truncation)]
      let d = start_time.elapsed().as_millis() as u32;
      d
    };

    match result {
      Ok(_output) => {
        self.emitter.emit(WatcherEvent::Event(BundleEvent::BundleEnd(BundleEndEventData::new(
          self.index,
          self.output_path.clone(),
          duration,
        ))));
      }
      Err(errs) => {
        // Format errors as strings since BuildDiagnostic doesn't implement Clone
        let error_messages: Vec<String> = errs.iter().map(|e| format!("{e:?}")).collect();
        self.emitter.emit(WatcherEvent::Event(BundleEvent::Error(BundleErrorEventData::new(
          self.index,
          error_messages,
          self.cwd.clone(),
        ))));
      }
    }

    self.needs_rebuild.store(false, Ordering::Relaxed);

    Ok(())
  }

  /// Update the set of files to watch
  async fn update_watch_files(&self, files: &[ArcStr]) -> BuildResult<()> {
    let mut notify_watcher = self.notify_watcher.lock().await;
    let mut watcher_paths = notify_watcher.paths_mut();

    for file in files {
      let file_str = file.as_str();
      if self.watch_files.contains(file_str) {
        continue;
      }
      let path = Path::new(file_str);
      if path.exists()
        && pattern_filter::filter(
          self.watch_exclude.as_deref(),
          self.watch_include.as_deref(),
          file_str,
          self.cwd.to_string_lossy().as_ref(),
        )
        .inner()
      {
        self.watch_files.insert(file.clone());
        if path.exists() {
          tracing::debug!(name = "notify watch", path = ?path);
          watcher_paths.add(path, WatchMode::non_recursive()).map_err_to_unhandleable()?;
        }
      }
    }
    watcher_paths.commit().map_err_to_unhandleable()?;

    Ok(())
  }

  /// Close the bundler task
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&self) -> Result<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.close().await?;
    Ok(())
  }
}
