use crate::watch::event::{BundleEvent, WatcherChangeData, WatcherEvent};
use arcstr::ArcStr;
use notify::{Config, RecommendedWatcher, Watcher as NotifyWatcher, event::ModifyKind};
use rolldown_common::{NotifyOption, WatcherChangeKind};
use rolldown_error::BuildResult;
use rolldown_utils::dashmap::FxDashSet;
use std::{
  ops::Deref,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender, channel},
  },
  time::Duration,
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
  running: AtomicBool,
  watch_changes: FxDashSet<WatcherChangeData>,
  // Shared channel sender for file system events
  tx: Arc<Sender<WatcherChannelMsg>>,
  // Shared async-safe receiver for file system events
  rx: Arc<Mutex<Receiver<WatcherChannelMsg>>>,
  // Shared channel sender for execution commands
  exec_tx: Arc<Sender<ExecChannelMsg>>,
  // Shared async-safe receiver for execution commands
  exec_rx: Arc<Mutex<Receiver<ExecChannelMsg>>>,
  // debounce invalidating
  invalidating: AtomicBool,
  // Collection of shared bundler instances across watchers
  bundlers: Vec<Arc<Mutex<Bundler>>>,
}

impl WatcherImpl {
  #[allow(clippy::needless_pass_by_value)]
  pub fn new(
    // Accept shared bundler instances for concurrent access during watching
    bundlers: Vec<Arc<Mutex<Bundler>>>,
    notify_option: Option<NotifyOption>,
  ) -> Result<Self> {
    let (tx, rx) = channel();
    let (exec_tx, exec_rx) = channel();
    // Wrap channel endpoints in Arc for sharing across threads
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
      config
    };
    let notify_watcher = Arc::new(Mutex::new(RecommendedWatcher::new(
      move |res| {
        if let Err(e) = tx.send(WatcherChannelMsg::NotifyEvent(res)) {
          eprintln!("send watch event error {e:?}");
        }
      },
      watch_option,
    )?));
    let notify_watch_files = Arc::new(FxDashSet::default());
    let emitter = Arc::new(WatcherEmitter::new());

    let tasks = bundlers
      .iter()
      .map(|bundler| {
        WatcherTask::new(
          Arc::clone(bundler),
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
      running: AtomicBool::default(),
      watch_changes: FxDashSet::default(),
      rx: Arc::new(Mutex::new(rx)),
      tx: cloned_tx,
      exec_tx: Arc::new(exec_tx),
      exec_rx: Arc::new(Mutex::new(exec_rx)),
      invalidating: AtomicBool::default(),
      bundlers,
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn invalidate(&self, data: Option<WatcherChangeData>) {
    tracing::debug!(name= "watch invalidate", running = ?self.running.load(Ordering::Relaxed));

    if let Some(data) = data {
      self.watch_changes.insert(data);
    }

    if self.running.load(Ordering::Relaxed) || self.invalidating.load(Ordering::Relaxed) {
      return;
    }

    self.invalidating.store(true, Ordering::Relaxed);
    self.exec_tx.send(ExecChannelMsg::Exec).expect("send watcher exec cannel message error");
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn run(&self, changed_files: &[ArcStr]) -> BuildResult<()> {
    self.emitter.emit(WatcherEvent::Restart)?;

    self.running.store(true, Ordering::Relaxed);
    self.emitter.emit(WatcherEvent::Event(BundleEvent::Start))?;
    for task in &self.tasks {
      task.run(changed_files).await?;
    }

    self.invalidating.store(false, Ordering::Relaxed);
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
    let inner = self.notify_watcher.lock().await;
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
    let build_delay = {
      let mut build_delay: u32 = 0;
      for bundler in &self.bundlers {
        let bundler = bundler.lock().await;
        if let Some(delay) = bundler.options.watch.build_delay {
          if delay > build_delay {
            build_delay = delay;
          }
        }
      }
      build_delay
    };

    let _ = self.run(&[]).await;
    let future = async move {
      let exec_rx = self.exec_rx.lock().await;
      while let Ok(msg) = exec_rx.recv() {
        match msg {
          ExecChannelMsg::Exec => {
            tokio::time::sleep(Duration::from_millis(u64::from(build_delay))).await;
            tracing::debug!(name= "watcher invalidate", watch_changes = ?self.watch_changes);
            let watch_changes =
              self.watch_changes.iter().map(|v| v.deref().clone()).collect::<Vec<_>>();
            for change in &watch_changes {
              for task in &self.tasks {
                task.on_change(change.path.as_str(), change.kind).await;
                task.invalidate(change.path.as_str()).await;
              }
              self.watch_changes.remove(change);
            }
            let changed_files =
              watch_changes.iter().map(|item| item.path.clone()).collect::<Vec<_>>();
            let _ = self.run(&changed_files).await;
          }
          ExecChannelMsg::Close => break,
        }
      }
    };

    rolldown_utils::futures::block_on(future);
  }
}

#[tracing::instrument(level = "debug", skip(watcher))]
// Accept shared watcher reference for emitting file change events across threads
pub fn emit_change_event(watcher: &Arc<WatcherImpl>, path: &str, kind: WatcherChangeKind) {
  let _ = watcher
    .emitter
    .emit(WatcherEvent::Change(WatcherChangeData { path: path.into(), kind }))
    .map_err(|e| eprintln!("Rolldown internal error: {e:?}"));
}

#[tracing::instrument(level = "debug", skip_all)]
// Accept shared watcher for monitoring file changes in background thread
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
