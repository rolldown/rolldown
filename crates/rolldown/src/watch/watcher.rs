use arcstr::ArcStr;
use notify::{event::ModifyKind, Config, RecommendedWatcher, Watcher as NotifyWatcher};
use rolldown_common::{
  BundleEvent, NotifyOption, WatcherChangeData, WatcherChangeKind, WatcherEvent,
};
use rolldown_error::BuildResult;
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
  notify_watcher: Arc<Mutex<RecommendedWatcher>>,
  notify_watch_files: Arc<FxDashSet<ArcStr>>,
  running: AtomicBool,
  rerun: AtomicBool,
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
    let notify_watcher = Arc::new(Mutex::new(RecommendedWatcher::new(
      move |res| {
        if let Err(e) = tx.send(WatcherChannelMsg::NotifyEvent(res)) {
          eprintln!("send watch event error {e:?}");
        };
      },
      watch_option,
    )?));
    let notify_watch_files = Arc::new(FxDashSet::default());
    let emitter = Arc::new(WatcherEmitter::new());

    let tasks = bundlers
      .into_iter()
      .map(|bundler| {
        WatcherTask::new(
          bundler,
          Arc::clone(&emitter),
          Arc::clone(&notify_watcher),
          Arc::clone(&notify_watch_files),
        )
      })
      .collect();

    Ok(Self {
      tasks,
      emitter,
      notify_watcher,
      notify_watch_files,
      running: AtomicBool::default(),
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

    for task in &self.tasks {
      task.run().await?;
    }

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
    let mut inner = self.notify_watcher.lock().await;
    for path in self.notify_watch_files.iter() {
      tracing::debug!(name= "notify close ", path = ?path.as_str());
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
                    tracing::debug!(name= "notify updated content", path = ?id.as_ref(), content= ?std::fs::read_to_string(id.as_ref()).unwrap());
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
