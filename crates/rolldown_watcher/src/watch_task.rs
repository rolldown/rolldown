use arcstr::ArcStr;
use async_lock::Mutex as TokioMutex;
use rolldown::{BundleHandle, Bundler, BundlerBuilder, BundlerConfig};
use rolldown_common::{
  BundleMode, Log, LogLevel, NormalizedBundlerOptions, ScanMode, WatchPath, WatcherChangeKind,
};
use rolldown_error::{
  BatchedBuildDiagnostic, BuildDiagnostic, BuildResult, Diagnostic, DiagnosticOptions,
  filter_out_disabled_diagnostics,
};
use rolldown_fs_watcher::FsWatcher;
use rolldown_utils::{dashmap::FxDashSet, pattern_filter};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::event::{BundleEndEventData, BundleStartEventData, WatchErrorEventData};

oxc_index::define_index_type! {
  pub struct WatchTaskIdx = u32;
}

oxc_index::define_index_type! {
  /// Identity of a config group: all output tasks created from one input
  /// config share one filesystem watcher and one invalidation stream.
  pub struct WatchGroupIdx = u32;
}

/// Per-task data container that owns a bundler and shares its config group's
/// file-system watcher.
pub struct WatchTask {
  bundler: Arc<TokioMutex<Bundler>>,
  options: Arc<NormalizedBundlerOptions>,
  /// Shared with every sibling output task of the same config group so one
  /// save is delivered exactly once per group. The watcher's own registered
  /// path set is the group-wide one: a path a sibling already registered is
  /// skipped there without opening a native batch, which on macOS would
  /// restart the group's shared FSEvents stream and drop the events buffered
  /// while it is open.
  fs_watcher: Arc<std::sync::Mutex<FsWatcher>>,
  /// Paths THIS task watches. `mark_needs_rebuild` and `is_watched_file`
  /// consult it, so paths a sibling registered must land here too.
  watched_files: FxDashSet<WatchPath>,
  pub(crate) needs_rebuild: bool,
  closed: Arc<AtomicBool>,
}

impl WatchTask {
  pub(crate) fn new(
    config: BundlerConfig,
    fs_watcher: Arc<std::sync::Mutex<FsWatcher>>,
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
      fs_watcher,
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

    // Cleared before the render-phase registration, unlike main: a commit
    // failure there returns early, and a still-set flag would make the
    // coordinator's same-envelope rebuild loop rebuild this task forever.
    // See internal-docs/watch-mode/implementation.md.
    self.needs_rebuild = false;

    // Also register any files discovered during render/write phase
    self.update_watch_files(&new_watch_files)?;

    #[expect(clippy::cast_possible_truncation)]
    let duration = start_time.elapsed().as_millis() as u32;

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
      on_log.call(LogLevel::Warn, Log::from_rendered(&warning, rendered)).await?;
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
    fs_watcher: &std::sync::Mutex<FsWatcher>,
    watched_files: &FxDashSet<WatchPath>,
    options: &NormalizedBundlerOptions,
    files: &[ArcStr],
  ) -> BuildResult<()> {
    // Candidates are the paths new to THIS task. The group's shared
    // `FsWatcher` opens a native batch only for the ones no member has
    // registered yet: on macOS, opening a batch stops the FSEvents stream and
    // the commit restarts it from "now", dropping events delivered in
    // between, and the stream is shared by the whole group. Steady-state
    // rebuilds, where every path is already in `watched_files`, return before
    // locking the watcher.
    let cwd = options.cwd.to_string_lossy();
    let candidates: Vec<WatchPath> = files
      .iter()
      .map(|file| WatchPath::new(file.as_str(), &options.cwd))
      .filter(|watch_path| {
        let path = watch_path.as_path();
        !watched_files.contains(watch_path)
          && path.exists()
          && pattern_filter::filter(
            options.watch.exclude.as_deref(),
            options.watch.include.as_deref(),
            &path.to_string_lossy(),
            &cwd,
          )
          .inner()
      })
      .collect();
    if candidates.is_empty() {
      return Ok(());
    }

    let mut fs_watcher = fs_watcher.lock().expect("fs_watcher lock poisoned");
    // A path notify refuses is skipped (debug log) and offered again on the
    // next build, like main; the batch is always committed and a commit
    // failure is returned. See internal-docs/watch-mode/implementation.md.
    let result = fs_watcher.watch_paths(candidates.iter().map(WatchPath::as_path), |_| true);
    // Publish to this task exactly what the group watcher holds: paths a
    // sibling registered earlier, plus this batch's paths whose add and
    // commit both succeeded. Anything else stays out of both sets, so the next
    // build offers it again.
    for watch_path in candidates {
      if fs_watcher.is_registered(watch_path.as_path()) {
        watched_files.insert(watch_path);
      }
    }
    result
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
  pub(crate) async fn call_watch_change(
    &self,
    path: &str,
    kind: WatcherChangeKind,
  ) -> anyhow::Result<()> {
    let bundler = self.bundler.lock().await;
    if let Some(plugin_driver) =
      bundler.last_bundle_handle.as_ref().map(rolldown::BundleHandle::plugin_driver)
    {
      plugin_driver.watch_change(path, kind).await?;
    }
    Ok(())
  }

  /// Call close_watcher plugin hooks
  #[tracing::instrument(level = "debug", skip_all)]
  pub(crate) async fn call_hook_close_watcher(&self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.close_watcher().await
  }

  pub(crate) async fn current_bundle_close_identity(&self) -> Option<u64> {
    let bundler = self.bundler.lock().await;
    bundler.last_bundle_handle.as_ref().map(BundleHandle::close_identity)
  }

  /// Close the bundler (calls `closeBundle` plugin hook for the last built bundle, if any).
  #[tracing::instrument(level = "debug", skip_all)]
  pub(crate) async fn close(&self) -> anyhow::Result<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.close().await.map_err(Into::into)
  }

  /// Per task, not per group: the shared watcher's set also holds the paths
  /// only a sibling output watches.
  fn is_watched_file(&self, path: &str) -> bool {
    Path::new(path).ancestors().any(|ancestor| self.watched_files.contains(ancestor))
  }
}

fn resolve_output_path(cwd: &Path, output: &str) -> PathBuf {
  let output = Path::new(output);
  let path = if output.is_absolute() { output.to_path_buf() } else { cwd.join(output) };
  let absolute_path = if path.is_absolute() {
    path
  } else {
    std::env::current_dir().map_or(path.clone(), |current_dir| current_dir.join(path))
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
  use rolldown_fs_watcher::{PathsMut, WatcherBackend};
  use rolldown_workspace::TestDir;
  use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
      Mutex,
      atomic::{AtomicBool, AtomicUsize, Ordering},
    },
  };

  struct CommitFailingWatcher {
    commit_attempts: Arc<AtomicUsize>,
  }

  struct CommitFailingPaths {
    commit_attempts: Arc<AtomicUsize>,
    pending: Vec<PathBuf>,
  }

  impl PathsMut for CommitFailingPaths {
    fn add(&mut self, path: &Path) -> BuildResult<()> {
      self.pending.push(path.to_path_buf());
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      if self.commit_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
        return Err(anyhow::anyhow!("intentional watcher commit failure").into());
      }
      Ok(())
    }
  }

  impl WatcherBackend for CommitFailingWatcher {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      Box::new(CommitFailingPaths {
        commit_attempts: Arc::clone(&self.commit_attempts),
        pending: Vec::new(),
      })
    }
  }

  struct AddFailingWatcher {
    add_attempts: Arc<Mutex<Vec<PathBuf>>>,
    commit_attempts: Arc<AtomicUsize>,
    event_delivery_paused: Arc<AtomicBool>,
  }

  struct AddFailingPaths {
    add_attempts: Arc<Mutex<Vec<PathBuf>>>,
    commit_attempts: Arc<AtomicUsize>,
    event_delivery_paused: Arc<AtomicBool>,
  }

  struct AddFailingWatcherFixture {
    watcher: std::sync::Mutex<FsWatcher>,
    add_attempts: Arc<Mutex<Vec<PathBuf>>>,
    commit_attempts: Arc<AtomicUsize>,
    event_delivery_paused: Arc<AtomicBool>,
  }

  impl PathsMut for AddFailingPaths {
    fn add(&mut self, path: &Path) -> BuildResult<()> {
      self.add_attempts.lock().expect("add attempts lock").push(path.to_path_buf());
      if path.ends_with("fail.js") {
        return Err(anyhow::anyhow!("intentional watcher add failure").into());
      }
      Ok(())
    }

    fn commit(self: Box<Self>) -> BuildResult<()> {
      self.commit_attempts.fetch_add(1, Ordering::SeqCst);
      self.event_delivery_paused.store(false, Ordering::SeqCst);
      Ok(())
    }
  }

  impl WatcherBackend for AddFailingWatcher {
    fn paths_mut(&mut self) -> Box<dyn PathsMut + '_> {
      assert!(
        !self.event_delivery_paused.swap(true, Ordering::SeqCst),
        "the preceding paths transaction must have restarted event delivery"
      );
      Box::new(AddFailingPaths {
        add_attempts: Arc::clone(&self.add_attempts),
        commit_attempts: Arc::clone(&self.commit_attempts),
        event_delivery_paused: Arc::clone(&self.event_delivery_paused),
      })
    }
  }

  fn is_registered(watcher: &std::sync::Mutex<FsWatcher>, path: &Path) -> bool {
    watcher.lock().expect("watcher lock").is_registered(path)
  }

  fn create_watch_files(test_dir: &TestDir, names: &[&str]) -> Vec<ArcStr> {
    names
      .iter()
      .map(|name| {
        let file = test_dir.path().join(name);
        fs::write(&file, "export const value = 1;").expect("write input");
        ArcStr::from(
          dunce::canonicalize(file).expect("canonicalize input").to_string_lossy().as_ref(),
        )
      })
      .collect()
  }

  fn create_add_failing_watcher() -> AddFailingWatcherFixture {
    let add_attempts = Arc::new(Mutex::new(Vec::new()));
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let event_delivery_paused = Arc::new(AtomicBool::new(false));
    let watcher = FsWatcher::with_backend(Box::new(AddFailingWatcher {
      add_attempts: Arc::clone(&add_attempts),
      commit_attempts: Arc::clone(&commit_attempts),
      event_delivery_paused: Arc::clone(&event_delivery_paused),
    }));
    AddFailingWatcherFixture {
      watcher: std::sync::Mutex::new(watcher),
      add_attempts,
      commit_attempts,
      event_delivery_paused,
    }
  }

  /// Main parity: a path notify refuses is skipped, not a build error. Every
  /// other path is still added and the batch committed, and the refused path
  /// stays out of both sets so the next registration offers it again.
  #[test]
  fn refused_watch_add_is_skipped_commits_and_is_retried() {
    let test_dir = TestDir::new("rolldown-watch-registration");
    let watch_files = create_watch_files(&test_dir, &["before.js", "fail.js", "after.js"]);
    let options =
      NormalizedBundlerOptions { cwd: test_dir.path().to_path_buf(), ..Default::default() };
    let AddFailingWatcherFixture { watcher, add_attempts, commit_attempts, event_delivery_paused } =
      create_add_failing_watcher();
    let watched_files = FxDashSet::default();

    WatchTask::update_watch_files_from(&watcher, &watched_files, &options, &watch_files)
      .expect("a refused watcher addition must be skipped, not reported");

    // `FsWatcher` stages a batch from a hash set, so only the set of attempts is stable.
    let mut attempted = add_attempts.lock().expect("add attempts lock").clone();
    attempted.sort();
    let mut expected =
      watch_files.iter().map(|file| PathBuf::from(file.as_str())).collect::<Vec<_>>();
    expected.sort();
    assert_eq!(attempted, expected, "a refused add must not skip later paths");
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);
    assert!(
      !event_delivery_paused.load(Ordering::SeqCst),
      "commit must restart event delivery after a refused add"
    );
    assert!(watched_files.contains(Path::new(watch_files[0].as_str())));
    assert!(!watched_files.contains(Path::new(watch_files[1].as_str())));
    assert!(watched_files.contains(Path::new(watch_files[2].as_str())));
    assert!(is_registered(&watcher, Path::new(watch_files[0].as_str())));
    assert!(
      !is_registered(&watcher, Path::new(watch_files[1].as_str())),
      "a refused add must stay unregistered for the whole group so the next build re-adds it"
    );
    assert!(is_registered(&watcher, Path::new(watch_files[2].as_str())));

    // The next registration offers only the skipped path again.
    WatchTask::update_watch_files_from(&watcher, &watched_files, &options, &watch_files)
      .expect("a refused watcher addition must be skipped again");
    let add_attempts = add_attempts.lock().expect("add attempts lock");
    assert_eq!(add_attempts.len(), watch_files.len() + 1);
    assert_eq!(add_attempts.last().map(PathBuf::as_path), Some(Path::new(watch_files[1].as_str())));
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 2);
  }

  #[test]
  fn failed_watch_commit_is_not_published_and_is_retried() {
    let test_dir = TestDir::new("rolldown-watch-registration");
    let file = test_dir.path().join("input.js");
    fs::write(&file, "export const value = 1;").expect("write input");
    let file = dunce::canonicalize(file).expect("canonicalize input");
    let watch_file = ArcStr::from(file.to_string_lossy().as_ref());
    let options = NormalizedBundlerOptions {
      cwd: file.parent().expect("input has parent").to_path_buf(),
      ..Default::default()
    };
    let commit_attempts = Arc::new(AtomicUsize::new(0));
    let watcher = FsWatcher::with_backend(Box::new(CommitFailingWatcher {
      commit_attempts: Arc::clone(&commit_attempts),
    }));
    let watcher = std::sync::Mutex::new(watcher);
    let watched_files = FxDashSet::default();

    let first = WatchTask::update_watch_files_from(
      &watcher,
      &watched_files,
      &options,
      std::slice::from_ref(&watch_file),
    );
    assert!(first.is_err());
    assert!(!watched_files.contains(Path::new(watch_file.as_str())));
    assert!(
      !is_registered(&watcher, Path::new(watch_file.as_str())),
      "a failed commit must not publish the path to the group set, \
       or the retry would adopt an unregistered path"
    );
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 1);

    WatchTask::update_watch_files_from(
      &watcher,
      &watched_files,
      &options,
      std::slice::from_ref(&watch_file),
    )
    .expect("second watcher commit should retry and succeed");
    assert!(watched_files.contains(Path::new(watch_file.as_str())));
    assert!(is_registered(&watcher, Path::new(watch_file.as_str())));
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 2);

    WatchTask::update_watch_files_from(
      &watcher,
      &watched_files,
      &options,
      std::slice::from_ref(&watch_file),
    )
    .expect("already-registered batch must succeed");
    // An empty batch must NOT open a transaction: on macOS the open/commit
    // cycle pauses the group's shared FSEvents stream and drops the events
    // delivered in between.
    assert_eq!(commit_attempts.load(Ordering::SeqCst), 2);
  }

  #[test]
  fn relative_output_path_is_resolved_and_normalized() {
    let cwd = std::env::current_dir().expect("current directory");
    assert_eq!(resolve_output_path(&cwd, "./nested/../dist/out.js"), cwd.join("dist/out.js"));

    let absolute = cwd.join("absolute.js");
    assert_eq!(resolve_output_path(&cwd, absolute.to_string_lossy().as_ref()), absolute);
  }
}
