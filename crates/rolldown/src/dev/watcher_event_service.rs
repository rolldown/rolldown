use rolldown_utils::indexmap::FxIndexSet;
use rolldown_watcher::FileChangeResult;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::dev::{
  build_driver::SharedBuildDriver, dev_context::SharedDevContext,
  watcher_event_handler::WatcherEventHandler,
};

pub enum WatcherEventServiceMsg {
  FileChange(FileChangeResult),
  Close,
}

pub type WatcherEventServiceTx = UnboundedSender<WatcherEventServiceMsg>;
pub type WatcherEventServiceRx = UnboundedReceiver<WatcherEventServiceMsg>;

pub struct WatcherEventService {
  pub build_driver: SharedBuildDriver,
  pub rx: WatcherEventServiceRx,
  pub tx: WatcherEventServiceTx,
  pub ctx: SharedDevContext,
}

impl WatcherEventService {
  pub fn new(build_driver: SharedBuildDriver, ctx: SharedDevContext) -> Self {
    let (tx, rx) = unbounded_channel::<WatcherEventServiceMsg>();
    Self { build_driver, ctx, rx, tx }
  }

  pub fn create_event_handler(&self) -> WatcherEventHandler {
    WatcherEventHandler { service_tx: self.tx.clone() }
  }

  pub fn tx(&self) -> &WatcherEventServiceTx {
    &self.tx
  }

  pub async fn run(mut self) {
    while let Some(msg) = {
      tracing::trace!("`BuildService` is waiting for messages.");
      self.rx.recv().await
    } {
      match msg {
        WatcherEventServiceMsg::FileChange(file_change_result) => match file_change_result {
          Ok(batched_events) => {
            tracing::debug!(target: "hmr", "Received batched events: {:#?}", batched_events);
            // TODO: using a IndexSet here will cause changes like [a.js, b.js, a.js] to be [a.js, b.js].
            // Not sure if we want this behavior for hmr scenario.
            let mut changed_files = FxIndexSet::default();
            batched_events.into_iter().for_each(|batched_event| match &batched_event.detail.kind {
              notify::EventKind::Modify(_modify_kind) => {
                changed_files.extend(batched_event.detail.paths);
              }
              _ => {}
            });

            self
              .build_driver
              .register_changed_files(changed_files.clone().into_iter().collect())
              .await;
            if self.ctx.options.eager_rebuild {
              self.build_driver.schedule_build_if_stale().await.expect("Should handle the error");
            }

            let changed_files =
              changed_files.into_iter().map(|file| file.to_string_lossy().to_string()).collect();

            self
              .build_driver
              .generate_hmr_updates(changed_files)
              .await
              .expect("Should handle the error");
          }
          Err(e) => {
            eprintln!("notify error: {e:?}");
          }
        },
        WatcherEventServiceMsg::Close => {
          break;
        }
      }
    }
  }
}
