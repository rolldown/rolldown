use rolldown_utils::indexmap::FxIndexSet;
use rolldown_watcher::FileChangeResult;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::dev::{
  build_driver::SharedBuildDriver, dev_context::SharedDevContext,
  watcher_event_handler::WatcherEventHandler,
};

pub enum BuildMessage {
  WatchEvent(FileChangeResult),
  BuildFinish,
}

pub type BuildChannelTx = UnboundedSender<BuildMessage>;
pub type BuildChannelRx = UnboundedReceiver<BuildMessage>;

pub struct BuildDriverService {
  pub build_driver: SharedBuildDriver,
  pub rx: BuildChannelRx,
  pub ctx: SharedDevContext,
}

impl BuildDriverService {
  pub fn new(build_driver: SharedBuildDriver, ctx: SharedDevContext, rx: BuildChannelRx) -> Self {
    Self { build_driver, ctx, rx }
  }

  pub fn create_watcher_event_handler(&self) -> WatcherEventHandler {
    WatcherEventHandler { service_tx: self.ctx.build_channel_tx.clone() }
  }

  pub async fn run(mut self) {
    while let Some(msg) = {
      tracing::trace!("`BuildService` is waiting for messages.");
      self.rx.recv().await
    } {
      match msg {
        BuildMessage::WatchEvent(watch_event) => match watch_event {
          Ok(batched_events) => {
            tracing::debug!(target: "hmr", "Received batched events: {:#?}", batched_events);

            let mut changed_files = FxIndexSet::default();
            batched_events.into_iter().for_each(|batched_event| match &batched_event.detail.kind {
              #[cfg(target_os = "macos")]
              notify::EventKind::Modify(notify::event::ModifyKind::Metadata(_))
                if !self.ctx.options.use_polling =>
              {
                // When using kqueue on mac, ignore metadata changes as it happens frequently and doesn't affect the build in most cases
                // Note that when using polling, we shouldn't ignore metadata changes as the polling watcher prefer to emit them over
                // content change events
              }
              notify::EventKind::Modify(_modify_kind) => {
                changed_files.extend(batched_event.detail.paths);
              }
              _ => {}
            });

            self.build_driver.handle_file_changes(changed_files).await;
            self
              .build_driver
              .schedule_build_if_stale()
              .await
              .expect("FIXME: should handle this error");
          }
          Err(e) => {
            eprintln!("notify error: {e:?}");
          }
        },
        BuildMessage::BuildFinish => {
          self
            .build_driver
            .schedule_build_if_stale()
            .await
            .expect("FIXME: should handle this error");
        }
      }
    }
  }
}
