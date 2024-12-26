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
enum ExecChannelMsg {
  Exec,
  Close,
}
pub struct WatcherImpl {
  pub emitter: SharedWatcherEmitter,
  tasks: Vec<WatcherTask>,
  notify_watcher: Arc<Mutex<RecommendedWatcher>>,
  notify_watch_files: Arc<FxDashSet<ArcStr>>,
  running: AtomicBool,
  watch_changes: FxDashSet<WatcherChangeData>,
  tx: Arc<Sender<WatcherChannelMsg>>,
  rx: Arc<Mutex<Receiver<WatcherChannelMsg>>>,
  exec_tx: Arc<Sender<ExecChannelMsg>>,
  exec_rx: Arc<Mutex<Receiver<ExecChannelMsg>>>,
}

impl WatcherImpl {
  #[allow(clippy::needless_pass_by_value)]
  pub fn new(
    bundlers: Vec<Arc<Mutex<Bundler>>>,
    notify_option: Option<NotifyOption>,
  ) -> Result<Self> {
    let (tx, rx) = channel();
    let (exec_tx, exec_rx) = channel();
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
      watch_changes: FxDashSet::default(),
      rx: Arc::new(Mutex::new(rx)),
      tx: cloned_tx,
      exec_tx: Arc::new(exec_tx),
      exec_rx: Arc::new(Mutex::new(exec_rx)),
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn invalidate(&self, data: Option<WatcherChangeData>) {
    tracing::debug!(name= "watch invalidate", running = ?self.running.load(Ordering::Relaxed));

    if let Some(data) = data {
      self.watch_changes.insert(data);
    }

    if self.running.load(Ordering::Relaxed) {
      return;
    }
    self.exec_tx.send(ExecChannelMsg::Exec).unwrap();
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

    if !self.watch_changes.is_empty() {
      self.invalidate(None);
    }

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&self) -> anyhow::Result<()> {
    // close channel
    self.tx.send(WatcherChannelMsg::Close)?;
    self.exec_tx.send(ExecChannelMsg::Close)?;
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
    let future = async move {
      while let Ok(msg) = self.exec_rx.lock().await.recv() {
        match msg {
          ExecChannelMsg::Exec => {
            for change in self.watch_changes.iter() {
              for task in &self.tasks {
                task.on_change(change.path.as_str(), change.kind).await;
                task.invalidate(change.path.as_str());
              }
            }
            self.watch_changes.clear();
            let _ = self.run().await;
          }
          ExecChannelMsg::Close => break,
        }
      }
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
}

#[tracing::instrument(level = "debug", skip(watcher))]
pub fn emit_change_event(watcher: &Arc<WatcherImpl>, path: &str, kind: WatcherChangeKind) {
  let _ = watcher
    .emitter
    .emit(WatcherEvent::Change(WatcherChangeData { path: path.into(), kind }))
    .map_err(|e| eprintln!("Rolldown internal error: {e:?}"));
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
                if let Some(kind) = match event.kind {
                  notify::EventKind::Create(_) => Some(WatcherChangeKind::Create),
                  notify::EventKind::Modify(
                    ModifyKind::Data(_) | ModifyKind::Any, /* windows*/
                  ) => {
                    tracing::debug!(name= "notify updated content", path = ?id.as_ref(), content= ?std::fs::read_to_string(id.as_ref()).unwrap());
                    Some(WatcherChangeKind::Update)
                  }
                  notify::EventKind::Remove(_) => Some(WatcherChangeKind::Delete),
                  _ => None,
                } {
                  emit_change_event(&watcher, id.as_ref(), kind);
                  watcher.invalidate(Some(WatcherChangeData { path: id.into(), kind }));
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
  tokio::spawn(future);
}
