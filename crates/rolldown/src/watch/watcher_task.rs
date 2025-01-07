use std::{
  path::Path,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  time::Instant,
};

use crate::{Bundler, SharedOptions};

use super::emitter::SharedWatcherEmitter;
use arcstr::ArcStr;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rolldown_common::{
  BundleEndEventData, BundleEvent, OutputsDiagnostics, WatcherChangeKind, WatcherEvent,
};
use rolldown_error::{BuildDiagnostic, BuildResult, ResultExt};
use rolldown_utils::{dashmap::FxDashSet, pattern_filter};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

pub struct WatcherTask {
  pub emitter: SharedWatcherEmitter,
  bundler: Arc<Mutex<Bundler>>,
  pub invalidate_flag: AtomicBool,
  notify_watcher: Arc<Mutex<RecommendedWatcher>>,
  notify_watch_files: Arc<FxDashSet<ArcStr>>,
  pub watch_files: FxDashSet<ArcStr>,
}

impl WatcherTask {
  pub fn new(
    bundler: Arc<Mutex<Bundler>>,
    emitter: SharedWatcherEmitter,
    notify_watcher: Arc<Mutex<RecommendedWatcher>>,
    notify_watched_files: Arc<FxDashSet<ArcStr>>,
  ) -> Self {
    Self {
      emitter,
      bundler,
      invalidate_flag: AtomicBool::new(true),
      watch_files: FxDashSet::default(),
      notify_watcher,
      notify_watch_files: notify_watched_files,
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn run(&self) -> BuildResult<()> {
    if !self.invalidate_flag.load(Ordering::Relaxed) {
      return Ok(());
    }
    let mut bundler = self.bundler.lock().await;

    let start_time = Instant::now();

    self.emitter.emit(WatcherEvent::Event(BundleEvent::BundleStart))?;

    bundler.plugin_driver.clear();

    let result = {
      let result = bundler.scan().await;
      // FIXME(hyf0): probably should have a more official API/better way to get watch files
      self.watch_files(&bundler.plugin_driver.watch_files, &bundler.options).await?;
      match result {
        Ok(scan_stage_output) => {
          if bundler.options.watch.skip_write {
            Ok(())
          } else {
            // avoid watching scan stage files twice
            bundler.plugin_driver.watch_files.clear();
            let output = bundler.bundle_write(scan_stage_output).await;
            self.watch_files(&bundler.plugin_driver.watch_files, &bundler.options).await?;
            match output {
              Ok(_) => Ok(()),
              Err(errs) => Err(errs),
            }
          }
        }
        Err(errs) => Err(errs),
      }
    };

    match result {
      Ok(()) => {
        self.emitter.emit(WatcherEvent::Event(BundleEvent::BundleEnd(BundleEndEventData {
          output: bundler
            .options
            .cwd
            .join(bundler.options.file.as_ref().unwrap_or(&bundler.options.out_dir))
            .to_string_lossy()
            .to_string(),
          #[allow(clippy::cast_possible_truncation)]
          duration: start_time.elapsed().as_millis() as u32,
        })))?;
      }
      Err(errs) => {
        self.emitter.emit(WatcherEvent::Event(BundleEvent::Error(OutputsDiagnostics {
          diagnostics: errs.into_vec(),
          cwd: bundler.options.cwd.clone(),
        })))?;
      }
    }

    self.invalidate_flag.store(false, Ordering::Relaxed);

    Ok(())
  }

  async fn watch_files(
    &self,
    files: &Arc<FxDashSet<ArcStr>>,
    options: &SharedOptions,
  ) -> BuildResult<()> {
    let mut notify_watcher = self.notify_watcher.lock().await;

    for file in files.iter() {
      if self.watch_files.contains(file.as_str()) {
        continue;
      }
      let path = Path::new(file.as_str());
      if path.exists() {
        let normalized_path = path.relative(&options.cwd);
        let normalized_id = normalized_path.to_string_lossy();
        if pattern_filter::filter(
          options.watch.exclude.as_deref(),
          options.watch.include.as_deref(),
          file.as_str(),
          &normalized_id,
        )
        .inner()
        {
          self.watch_files.insert(file.clone());
          // we should skip the file that is already watched, here here some reasons:
          // - The watching files has a ms level overhead.
          // - Watching the same files multiple times will cost more overhead.
          // TODO: tracking https://github.com/notify-rs/notify/issues/653
          if self.notify_watch_files.contains(file.as_str()) {
            continue;
          }
          let path = Path::new(file.as_str());
          if path.exists() {
            tracing::debug!(name= "notify watch ", path = ?path);
            notify_watcher.watch(path, RecursiveMode::Recursive).map_err_to_unhandleable()?;
            self.notify_watch_files.insert(file.clone());
          }
        }
      }
    }

    // The inner mutex should be dropped to avoid deadlock with bundler lock at `Watcher::close`
    std::mem::drop(notify_watcher);

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&self) -> anyhow::Result<()> {
    let bundler = self.bundler.lock().await;
    bundler.plugin_driver.close_watcher().await?;
    Ok(())
  }

  pub fn invalidate(&self, path: &str) {
    // invalidate the watcher task if the changed file is in the watch list
    if self.watch_files.contains(path) {
      self.invalidate_flag.store(true, Ordering::Relaxed);
    }
  }

  #[tracing::instrument(level = "debug", skip(self))]
  pub async fn on_change(&self, path: &str, kind: WatcherChangeKind) {
    let bundler = self.bundler.lock().await;
    let _ = bundler.plugin_driver.watch_change(path, kind).await.map_err(|e| {
      self.emitter.emit(WatcherEvent::Event(BundleEvent::Error(OutputsDiagnostics {
        diagnostics: vec![BuildDiagnostic::unhandleable_error(e)],
        cwd: bundler.options.cwd.clone(),
      })))
    });
  }
}
