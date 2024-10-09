use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use notify::{
  event::ModifyKind, Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use rustc_hash::FxHashSet;
use tokio::sync::mpsc::channel;

use crate::Bundler;

use super::emitter::{
  BundleEventKind, SharedWatcherEmitter, WatcherChange, WatcherChangeKind, WatcherEvent,
};
use anyhow::Result;

pub struct Watcher {
  inner: RecommendedWatcher,
  emitter: SharedWatcherEmitter,
  running: bool,
  rerun: bool,
  watch_files: FxHashSet<ArcStr>,
}

impl Watcher {
  pub fn new(emitter: SharedWatcherEmitter, inner: RecommendedWatcher) -> Self {
    Self { inner, emitter, running: false, watch_files: FxHashSet::default(), rerun: false }
  }

  #[allow(unused_must_use)]
  pub fn invalidate(&mut self, bundler: &mut Bundler) {
    if self.running {
      self.rerun = true;
      return;
    }
    if self.rerun {
      return;
    }

    #[cfg(target_family = "wasm")]
    {
      futures::executor::block_on(async {
        self.rerun = false;
        self.run(bundler).await;
      });
    }
    #[cfg(not(target_family = "wasm"))]
    {
      tokio::task::block_in_place(move || {
        tokio::runtime::Handle::current().block_on(async move {
          self.rerun = false;
          self.run(bundler).await;
        });
      });
    }
  }

  pub async fn run(&mut self, bundler: &mut Bundler) -> Result<()> {
    self.running = true;
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::Start.into());

    self.emitter.emit(WatcherEvent::Event, BundleEventKind::BundleStart.into());
    bundler.plugin_driver = bundler.plugin_driver.new_shared_from_self();
    bundler.file_emitter.clear();

    // TODO support skipWrite option
    let output = bundler.write().await?;
    for file in &output.watch_files {
      let path = Path::new(file.as_str());
      if path.exists() {
        self.inner.watch(path, RecursiveMode::Recursive)?;
        self.watch_files.insert(file.clone());
      }
    }
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::BundleEnd.into());

    self.running = false;
    self.emitter.emit(WatcherEvent::Event, BundleEventKind::End.into());

    Ok(())
  }

  #[allow(dead_code)]
  pub fn close(&mut self) {
    for path in &self.watch_files {
      self.inner.unwatch(Path::new(path.as_str())).expect("should unwatch");
    }
  }
}

pub async fn setup_watcher(emitter: SharedWatcherEmitter, bundler: &mut Bundler) -> Result<()> {
  let (tx, mut rx) = channel(100);

  let inner = RecommendedWatcher::new(
    move |res| {
      futures::executor::block_on(async {
        tx.send(res).await.unwrap();
      });
    },
    Config::default(),
  )?;

  let mut watcher = Watcher::new(Arc::clone(&emitter), inner);

  watcher.run(bundler).await?;

  // TODO handle close gracefully
  //   emitter.on(
  //     WatcherEvent::Close,
  //     Box::new(|_| {
  //       watcher.close();
  //       rx.close();
  //     }),
  //   );

  while let Some(res) = rx.recv().await {
    match res {
      Ok(event) => {
        for path in event.paths {
          match event.kind {
            notify::EventKind::Create(_) => {
              emitter.emit(
                WatcherEvent::Change,
                WatcherChange { path, kind: WatcherChangeKind::Create }.into(),
              );
            }
            notify::EventKind::Modify(ModifyKind::Data(_)) => {
              emitter.emit(
                WatcherEvent::Change,
                WatcherChange { path, kind: WatcherChangeKind::Update }.into(),
              );
              watcher.invalidate(bundler);
            }
            notify::EventKind::Remove(_) => {
              emitter.emit(
                WatcherEvent::Change,
                WatcherChange { path, kind: WatcherChangeKind::Delete }.into(),
              );
            }
            _ => {}
          }
        }
      }
      Err(e) => panic!("watch error: {e:?}"),
    }
  }

  Ok(())
}
