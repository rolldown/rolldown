use std::{
  path::Path,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};

use arcstr::ArcStr;
use dashmap::DashSet;
use futures::{
  channel::mpsc::{channel, Receiver},
  SinkExt, StreamExt,
};
use notify::{
  event::ModifyKind, Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use rolldown_common::{
  BundleEventKind, WatcherChange, WatcherChangeKind, WatcherEvent, WatcherEventData,
};
use rolldown_plugin::SharedPluginDriver;
use tokio::sync::Mutex;

use crate::Bundler;

use anyhow::Result;

use super::emitter::{SharedWatcherEmitter, WatcherEmitter};

pub struct Watcher {
  pub(crate) emitter: SharedWatcherEmitter,
  inner: Arc<Mutex<RecommendedWatcher>>,
  running: AtomicBool,
  rerun: AtomicBool,
  watch_files: DashSet<ArcStr>,
  rx: Arc<Mutex<Receiver<notify::Result<notify::Event>>>>,
}

impl Watcher {
  pub fn new() -> Result<Self> {
    let (mut tx, rx) = channel(100);
    let inner = RecommendedWatcher::new(
      move |res| {
        futures::executor::block_on(async {
          match tx.send(res).await {
            Ok(_) => {}
            Err(_) => { /* the channel maybe closed */ }
          };
        });
      },
      Config::default(),
    )?;

    Ok(Self {
      emitter: Arc::new(WatcherEmitter::new()),
      inner: Arc::new(Mutex::new(inner)),
      running: AtomicBool::default(),
      watch_files: DashSet::default(),
      rerun: AtomicBool::default(),
      rx: Arc::new(Mutex::new(rx)),
    })
  }

  #[allow(unused_must_use)]
  pub async fn invalidate(&self, bundler: Arc<Mutex<Bundler>>) {
    if self.running.load(Ordering::Relaxed) {
      self.rerun.store(true, Ordering::Relaxed);
      return;
    }
    if self.rerun.load(Ordering::Relaxed) {
      return;
    }

    #[cfg(target_family = "wasm")]
    {
      futures::executor::block_on(async {
        self.rerun.store(false, Ordering::Relaxed);
        self.run(bundler).await;
      });
    }
    #[cfg(not(target_family = "wasm"))]
    {
      tokio::task::block_in_place(move || {
        tokio::runtime::Handle::current().block_on(async move {
          self.rerun.store(false, Ordering::Relaxed);
          self.run(bundler).await;
        });
      });
    }
  }

  pub async fn run(&self, bundler: Arc<Mutex<Bundler>>) -> Result<()> {
    let mut bundler =
      bundler.try_lock().expect("Failed to lock the bundler. Is another operation in progress?");

    self.running.store(true, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::Start.into());

    self.emitter.emit(WatcherEvent::Event, BundleEventKind::BundleStart.into());
    bundler.plugin_driver = bundler.plugin_driver.new_shared_from_self();
    bundler.file_emitter.clear();

    // TODO support skipWrite option
    let output = bundler.write().await?;
    let mut inner = self.inner.try_lock().expect("Failed to lock the notify watcher.");
    for file in &output.watch_files {
      let path = Path::new(file.as_str());
      if path.exists() {
        inner.watch(path, RecursiveMode::Recursive)?;
        self.watch_files.insert(file.clone());
      }
    }
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::BundleEnd.into());

    self.running.store(false, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::End.into());

    Ok(())
  }

  pub fn watch_file(&self, path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.exists() {
      let mut inner = self.inner.try_lock().expect("Failed to lock the notify watcher.");
      inner.watch(path, RecursiveMode::Recursive)?;
      self.watch_files.insert(path.to_string_lossy().into());
    }
    Ok(())
  }

  pub fn close(&self) {
    let mut rx = self.rx.try_lock().expect("Failed to lock the watcher receiver. ");
    // close channel
    rx.close();
    // stop watching files
    let mut inner = self.inner.try_lock().expect("Failed to lock the notify watcher.");
    for path in self.watch_files.iter() {
      inner.unwatch(Path::new(path.as_str())).expect("should unwatch");
    }
    // emit close event
    self.emitter.emit(WatcherEvent::Close, WatcherEventData::default());
  }
}

pub async fn on_change(
  emitter: &SharedWatcherEmitter,
  plugin_driver: &SharedPluginDriver,
  path: &str,
  kind: WatcherChangeKind,
) {
  emitter.emit(WatcherEvent::Change, WatcherChange { path: path.into(), kind }.into());
  plugin_driver.watch_change(path, kind).await.expect("call watch change failed");
}

pub fn wait_for_change(watcher: Arc<Watcher>, bundler: Arc<Mutex<Bundler>>) {
  let cloned_bundler = Arc::clone(&bundler);
  let bundler_guard = cloned_bundler.try_lock().expect("Failed to lock the bundler. ");
  let plugin_driver = Arc::clone(&bundler_guard.plugin_driver);

  tokio::spawn(async move {
    let mut rx = watcher.rx.try_lock().expect("Failed to lock the watcher receiver. ");
    while let Some(res) = rx.next().await {
      match res {
        Ok(event) => {
          for path in event.paths {
            let id = path.to_string_lossy();
            match event.kind {
              notify::EventKind::Create(_) => {
                on_change(&watcher.emitter, &plugin_driver, id.as_ref(), WatcherChangeKind::Create)
                  .await;
              }
              notify::EventKind::Modify(
                ModifyKind::Data(_) | ModifyKind::Any, /* windows*/
              ) => {
                on_change(&watcher.emitter, &plugin_driver, id.as_ref(), WatcherChangeKind::Update)
                  .await;
                watcher.invalidate(Arc::clone(&bundler)).await;
              }
              notify::EventKind::Remove(_) => {
                on_change(&watcher.emitter, &plugin_driver, id.as_ref(), WatcherChangeKind::Delete)
                  .await;
              }
              _ => {}
            }
          }
        }
        Err(e) => {
          eprintln!("watcher error: {:?}", e);
        }
      }
    }
  });
}
