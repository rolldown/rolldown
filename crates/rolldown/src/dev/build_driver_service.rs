use rolldown_utils::indexmap::FxIndexSet;
use rolldown_watcher::FileChangeResult;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::dev::{
  build_driver::SharedBuildDriver, dev_context::SharedDevContext,
  watcher_event_handler::WatcherEventHandler,
};

pub enum BuildMessage {
  FileChange(FileChangeResult),
}

pub type BuildDriverServiceTx = UnboundedSender<BuildMessage>;
pub type BuildDriverServiceRx = UnboundedReceiver<BuildMessage>;

pub struct BuildDriverService {
  pub build_driver: SharedBuildDriver,
  pub rx: BuildDriverServiceRx,
  pub tx: BuildDriverServiceTx,
  pub ctx: SharedDevContext,
}

impl BuildDriverService {
  pub fn new(build_driver: SharedBuildDriver, ctx: SharedDevContext) -> Self {
    let (tx, rx) = unbounded_channel::<BuildMessage>();
    Self { build_driver, ctx, rx, tx }
  }

  pub fn create_watcher_event_handler(&self) -> WatcherEventHandler {
    WatcherEventHandler { service_tx: self.tx.clone() }
  }

  #[expect(clippy::print_stdout)]
  pub async fn run(mut self) {
    while let Some(msg) = {
      tracing::trace!("`BuildService` is waiting for messages.");
      self.rx.recv().await
    } {
      match msg {
        BuildMessage::FileChange(file_change_result) => match file_change_result {
          Ok(batched_events) => {
            tracing::debug!(target: "hmr", "Received batched events: {:#?}", batched_events);
            if option_env!("CI").is_some() {
              println!("[WatcherEventService]: Received batched events: {batched_events:#?}");
            }
            // TODO: using a IndexSet here will cause changes like [a.js, b.js, a.js] to be [a.js, b.js].
            // Not sure if we want this behavior for hmr scenario.
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
      }
    }
  }
}
