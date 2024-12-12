use std::{
  path::Path,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  time::Instant,
};

use crate::Bundler;

use super::emitter::SharedWatcherEmitter;
use arcstr::ArcStr;
use rolldown_common::{
  BundleEndEventData, BundleEvent, OutputsDiagnostics, WatcherChangeKind, WatcherEvent,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{dashmap::FxDashSet, pattern_filter};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

pub struct WatcherTask {
  pub emitter: SharedWatcherEmitter,
  bundler: Arc<Mutex<Bundler>>,
  invalidate: AtomicBool,
  pub watch_files: FxDashSet<ArcStr>,
}

impl WatcherTask {
  pub fn new(bundler: Arc<Mutex<Bundler>>, emitter: SharedWatcherEmitter) -> Self {
    Self { emitter, bundler, invalidate: AtomicBool::new(true), watch_files: FxDashSet::default() }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn run(&self) -> BuildResult<()> {
    if !self.invalidate.load(Ordering::Relaxed) {
      return Ok(());
    }
    let mut bundler = self.bundler.lock().await;

    let start_time = Instant::now();

    self.emitter.emit(WatcherEvent::Event(BundleEvent::BundleStart))?;

    bundler.plugin_driver.clear();

    let output = {
      if bundler.options.watch.skip_write {
        // TODO Here should be call scan
        bundler.generate().await
      } else {
        bundler.write().await
      }
    };

    // FIXME(hyf0): probably should have a more official API/better way to get watch files
    for file in bundler.plugin_driver.watch_files.iter() {
      if self.watch_files.contains(file.as_str()) {
        continue;
      }
      let path = Path::new(file.as_str());
      if path.exists() {
        let normalized_path = path.relative(&bundler.options.cwd);
        let normalized_id = normalized_path.to_string_lossy();
        if pattern_filter::filter(
          bundler.options.watch.exclude.as_deref(),
          bundler.options.watch.include.as_deref(),
          file.as_str(),
          &normalized_id,
        )
        .inner()
        {
          self.watch_files.insert(file.clone());
        }
      }
    }

    match output {
      Ok(_output) => {
        self.emitter.emit(WatcherEvent::Event(BundleEvent::BundleEnd(BundleEndEventData {
          output: bundler.options.cwd.join(&bundler.options.dir).to_string_lossy().to_string(),
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

    self.invalidate.store(false, Ordering::Relaxed);

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&self) -> anyhow::Result<()> {
    let bundler = self.bundler.lock().await;
    bundler.plugin_driver.close_watcher().await?;
    Ok(())
  }

  #[tracing::instrument(level = "debug", skip(self))]
  pub async fn on_change(&self, path: &str, kind: WatcherChangeKind) {
    // invalidate the watcher task if the changed file is in the watch list
    if self.watch_files.contains(path) {
      self.invalidate.store(true, Ordering::Relaxed);
    }

    let bundler = self.bundler.lock().await;
    let _ = bundler.plugin_driver.watch_change(path, kind).await.map_err(|e| {
      self.emitter.emit(WatcherEvent::Event(BundleEvent::Error(OutputsDiagnostics {
        diagnostics: vec![BuildDiagnostic::unhandleable_error(e)],
        cwd: bundler.options.cwd.clone(),
      })))
    });
  }
}
