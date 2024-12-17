use arcstr::ArcStr;
use dashmap::DashSet;
use notify::{
  event::ModifyKind, Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use rolldown_common::{
  BundleEvent, NotifyOption, WatcherChangeData, WatcherChangeKind, WatcherEvent,
};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_utils::dashmap::FxDashSet;
use std::{
  path::Path,
  sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{channel, Receiver, Sender},
    Arc,
  },
};
use tokio::sync::Mutex;

use crate::Bundler;

use anyhow::Result;

use super::{
  emitter::{SharedWatcherEmitter, WatcherEmitter},
  watcher_task::WatcherTask,
};

enum WatcherChannelMsg {
  NotifyEvent(notify::Result<notify::Event>),
  Close,
}

pub struct WatcherImpl {
  pub emitter: SharedWatcherEmitter,
  tasks: Vec<WatcherTask>,
  inner: Arc<Mutex<RecommendedWatcher>>,
  running: AtomicBool,
  rerun: AtomicBool,
  watch_files: FxDashSet<ArcStr>,
  tx: Arc<Sender<WatcherChannelMsg>>,
  rx: Arc<Mutex<Receiver<WatcherChannelMsg>>>,
}

impl WatcherImpl {
  #[allow(clippy::needless_pass_by_value)]
  pub fn new(
    bundlers: Vec<Arc<Mutex<Bundler>>>,
    notify_option: Option<NotifyOption>,
  ) -> Result<Self> {
    let (tx, rx) = channel();
    let tx = Arc::new(tx);
    let cloned_tx = Arc::clone(&tx);
    let watch_option = {
      let config = Config::default();
      if let Some(notify) = &notify_option {
        if let Some(poll_interval) = notify.poll_interval {
          config.with_poll_interval(poll_interval);
        }
        config.with_compare_contents(notify.compare_contents);
      }
      Config::default()
    };
    let inner = RecommendedWatcher::new(
      move |res| {
        if let Err(e) = tx.send(WatcherChannelMsg::NotifyEvent(res)) {
          eprintln!("send watch event error {e:?}");
        };
      },
      watch_option,
    )?;

    let emitter = Arc::new(WatcherEmitter::new());

    let tasks =
      bundlers.into_iter().map(|bundler| WatcherTask::new(bundler, Arc::clone(&emitter))).collect();

    Ok(Self {
      tasks,
      emitter,
      inner: Arc::new(Mutex::new(inner)),
      running: AtomicBool::default(),
      watch_files: DashSet::default(),
      rerun: AtomicBool::default(),
      rx: Arc::new(Mutex::new(rx)),
      tx: cloned_tx,
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
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

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn run(&self) -> BuildResult<()> {
    self.emitter.emit(WatcherEvent::ReStart)?;

    self.running.store(true, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event(BundleEvent::Start))?;

    let mut inner = self.inner.lock().await;

    for task in &self.tasks {
      task.run().await?;

      for file in task.watch_files.iter() {
        // we should skip the file that is already watched, here here some reasons:
        // - The watching files has a ms level overhead.
        // - Watching the same files multiple times will cost more overhead.
        // TODO: tracking https://github.com/notify-rs/notify/issues/653
        if self.watch_files.contains(file.as_str()) {
          continue;
        }
        let path = Path::new(file.as_str());
        if path.exists() {
          tracing::debug!(name= "notify watch ", path = ?path);
          inner.watch(path, RecursiveMode::Recursive).map_err_to_unhandleable()?;
          self.watch_files.insert(file.clone());
        }
      }
    }

    // The inner mutex should be dropped to avoid deadlock with bundler lock at `Watcher::close`
    std::mem::drop(inner);

    self.running.store(false, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event(BundleEvent::End))?;

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&self) -> anyhow::Result<()> {
    // close channel
    self.tx.send(WatcherChannelMsg::Close)?;
    // stop watching files
    // TODO the notify watcher should be dropped, because the stop method is private
    let mut inner = self.inner.lock().await;
    for path in self.watch_files.iter() {
      inner.unwatch(Path::new(path.as_str()))?;
    }
    // The inner mutex should be dropped to avoid deadlock with bundler lock at `Watcher::run`
    std::mem::drop(inner);
    // emit close event
    self.emitter.emit(WatcherEvent::Close)?;
    // call close watcher hook
    for task in &self.tasks {
      task.close().await?;
    }

    Ok(())
  }

  pub async fn start(&self) {
    let _ = self.run().await;
  }
}

#[tracing::instrument(level = "debug", skip(watcher))]
pub async fn on_change(watcher: &Arc<WatcherImpl>, path: &str, kind: WatcherChangeKind) {
  let _ = watcher
    .emitter
    .emit(WatcherEvent::Change(WatcherChangeData { path: path.into(), kind }))
    .map_err(|e| eprintln!("Rolldown internal error: {e:?}"));
  for task in &watcher.tasks {
    task.on_change(path, kind).await;
  }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn wait_for_change(watcher: Arc<WatcherImpl>) {
  let future = async move {
    let mut run = true;
    while run {
      let rx = watcher.rx.lock().await;
      match rx.recv() {
        Ok(msg) => match msg {
          WatcherChannelMsg::NotifyEvent(event) => match event {
            Ok(event) => {
              tracing::debug!(name= "notify event ", event = ?event);
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
            Err(e) => eprintln!("notify error: {e:?}"),
          },
          WatcherChannelMsg::Close => run = false,
        },
        Err(e) => {
          eprintln!("watcher receiver error: {e:?}");
        }
      }
    }
  };

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
