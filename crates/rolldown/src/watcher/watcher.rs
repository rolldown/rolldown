use arcstr::ArcStr;
use dashmap::DashSet;
use futures::{
  channel::mpsc::{channel, Receiver, Sender},
  SinkExt, StreamExt,
};
use notify::{
  event::ModifyKind, Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use rolldown_common::{
  BundleEventKind, WatcherChange, WatcherChangeKind, WatcherEvent, WatcherEventData,
};
use std::{
  path::Path,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};
use tokio::sync::Mutex;

use crate::Bundler;

use anyhow::Result;

use super::emitter::{SharedWatcherEmitter, WatcherEmitter};

pub struct Watcher {
  pub emitter: SharedWatcherEmitter,
  bundler: Arc<Mutex<Bundler>>,
  inner: Arc<Mutex<RecommendedWatcher>>,
  running: AtomicBool,
  rerun: AtomicBool,
  watch_files: DashSet<ArcStr>,
  tx: Arc<Mutex<Sender<notify::Result<notify::Event>>>>,
  rx: Arc<Mutex<Receiver<notify::Result<notify::Event>>>>,
}

impl Watcher {
  pub fn new(bundler: Arc<Mutex<Bundler>>) -> Result<Self> {
    let (tx, rx) = channel(100);
    let tx = Arc::new(Mutex::new(tx));
    let cloned_tx = Arc::clone(&tx);
    let inner = RecommendedWatcher::new(
      move |res| {
        let mut tx = tx.try_lock().expect("Failed to lock the watcher sender");
        futures::executor::block_on(async {
          if tx.is_closed() {
            return;
          }
          match tx.send(res).await {
            Ok(()) => {}
            Err(e) => {
              eprintln!("send watch event error {e:?}");
            }
          };
        });
      },
      Config::default(),
    )?;

    Ok(Self {
      emitter: Arc::new(WatcherEmitter::new()),
      bundler,
      inner: Arc::new(Mutex::new(inner)),
      running: AtomicBool::default(),
      watch_files: DashSet::default(),
      rerun: AtomicBool::default(),
      rx: Arc::new(Mutex::new(rx)),
      tx: cloned_tx,
    })
  }

  pub fn invalidate(&self) {
    if self.running.load(Ordering::Relaxed) {
      self.rerun.store(true, Ordering::Relaxed);
      return;
    }
    if self.rerun.load(Ordering::Relaxed) {
      return;
    }

    let future = async move {
      self.rerun.store(false, Ordering::Relaxed);
      let _ = self.run().await;
    };

    #[cfg(target_family = "wasm")]
    {
      futures::executor::block_on(future);
    }
    #[cfg(not(target_family = "wasm"))]
    {
      tokio::task::block_in_place(move || {
        tokio::runtime::Handle::current().block_on(future);
      });
    }
  }

  pub async fn run(&self) -> Result<()> {
    let mut bundler = self
      .bundler
      .try_lock()
      .expect("Failed to lock the bundler. Is another operation in progress?");
    self.emitter.emit(WatcherEvent::ReStart, WatcherEventData::default()).await?;

    self.running.store(true, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::Start.into()).await?;

    self.emitter.emit(WatcherEvent::Event, BundleEventKind::BundleStart.into()).await?;
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
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::BundleEnd.into()).await?;

    self.running.store(false, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::End.into()).await?;

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

  pub async fn close(&self) -> anyhow::Result<()> {
    // close channel
    let mut tx = self.tx.try_lock()?;
    let _ = tx.close().await;
    // stop watching files
    // TODO the notify watcher should be dropped, because the stop method is private
    let mut inner = self.inner.try_lock()?;
    for path in self.watch_files.iter() {
      inner.unwatch(Path::new(path.as_str()))?;
    }
    // emit close event
    self.emitter.emit(WatcherEvent::Close, WatcherEventData::default()).await?;
    // call close watcher hook
    let bundler = self.bundler.try_lock()?;
    bundler.plugin_driver.close_watcher().await?;

    Ok(())
  }
}

pub async fn on_change(watcher: &Arc<Watcher>, path: &str, kind: WatcherChangeKind) {
  let _ = watcher
    .emitter
    .emit(WatcherEvent::Change, WatcherChange { path: path.into(), kind }.into())
    .await
    .map_err(|e| eprintln!("Rolldown internal error: {e:?}"));
  let bundler = watcher.bundler.try_lock().expect("Failed to lock the bundler. ");
  let _ = bundler
    .plugin_driver
    .watch_change(path, kind)
    .await
    .map_err(|e| eprintln!("Rolldown internal error: {e:?}"));
}

pub fn wait_for_change(watcher: Arc<Watcher>) {
  let future = async move {
    let mut rx = watcher.rx.try_lock().expect("Failed to lock the watcher receiver. ");
    while let Some(res) = rx.next().await {
      match res {
        Ok(event) => {
          for path in event.paths {
            let id = path.to_string_lossy();
            match event.kind {
              notify::EventKind::Create(_) => {
                on_change(&watcher, id.as_ref(), WatcherChangeKind::Create).await;
              }
              notify::EventKind::Modify(
                ModifyKind::Data(_) | ModifyKind::Any, /* windows*/
              ) => {
                on_change(&watcher, id.as_ref(), WatcherChangeKind::Update).await;
                watcher.invalidate();
              }
              notify::EventKind::Remove(_) => {
                on_change(&watcher, id.as_ref(), WatcherChangeKind::Delete).await;
              }
              _ => {}
            }
          }
        }
        Err(e) => {
          eprintln!("watcher receiver error: {e:?}");
        }
      }
    }
  };

  // TODO the spawn task should be dropped

  #[cfg(target_family = "wasm")]
  {
    let handle = tokio::runtime::Handle::current();
    // could not block_on/spawn the main thread in WASI
    std::thread::spawn(move || {
      handle.spawn(future);
    });
  }
  #[cfg(not(target_family = "wasm"))]
  tokio::spawn(future);
}
