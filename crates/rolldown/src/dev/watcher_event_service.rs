use rolldown_watcher::FileChangeResult;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::dev::{build_driver::SharedBuildDriver, watcher_event_handler::WatcherEventHandler};

pub enum WatcherEventServiceMsg {
  FileChange(FileChangeResult),
}

pub type WatcherEventServiceTx = UnboundedSender<WatcherEventServiceMsg>;
pub type WatcherEventServiceRx = UnboundedReceiver<WatcherEventServiceMsg>;

pub struct WatcherEventService {
  pub ctx: SharedBuildDriver,
  pub rx: WatcherEventServiceRx,
  pub tx: WatcherEventServiceTx,
}

impl WatcherEventService {
  pub fn new(ctx: SharedBuildDriver) -> Self {
    let (tx, rx) = unbounded_channel::<WatcherEventServiceMsg>();
    Self { ctx, rx, tx }
  }

  pub fn create_event_handler(&self) -> WatcherEventHandler {
    WatcherEventHandler { service_tx: self.tx.clone() }
  }

  pub async fn run(mut self) {
    while let Some(msg) = {
      tracing::trace!("`BuildService` is waiting for messages.");
      self.rx.recv().await
    } {
      match msg {
        WatcherEventServiceMsg::FileChange(file_change_result) => match file_change_result {
          Ok(batched_events) => {
            let changed_files = batched_events
              .into_iter()
              .flat_map(|batched_event| match &batched_event.detail.kind {
                notify::EventKind::Modify(_modify_kind) => batched_event.detail.paths,
                _ => {
                  vec![]
                }
              })
              .collect::<Vec<_>>();

            self.ctx.schedule_build(changed_files).await;
          }
          Err(e) => {
            eprintln!("notify error: {e:?}");
          }
        },
      }
    }
  }
}
