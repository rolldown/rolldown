use std::{collections::HashMap, sync::Arc};

use napi_derive::napi;

use crate::utils::handle_result;

use super::js_callback::MaybeAsyncJsCallback;
use crate::types::js_callback::MaybeAsyncJsCallbackExt;

#[napi]
pub struct BindingWatcher {
  inner: Arc<rolldown::Watcher>,
}

#[napi]
impl BindingWatcher {
  pub fn new(inner: Arc<rolldown::Watcher>) -> Self {
    Self { inner }
  }

  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    handle_result(self.inner.close().await)
  }

  #[napi(
    ts_args_type = "event: BindingWatcherEvent, listener: (data?: Record<string, string>) => void"
  )]
  pub fn on(
    &self,
    event: BindingWatcherEvent,
    listener: MaybeAsyncJsCallback<Option<HashMap<String, String>>, ()>,
  ) -> napi::Result<()> {
    self.inner.emitter.on(
      event.into(),
      Box::new(move |data| {
        let listener = Arc::clone(&listener);
        let data = data.inner().clone();
        Box::pin(async move { listener.await_call(data).await.map_err(anyhow::Error::from) })
      }),
    );
    Ok(())
  }
}

#[napi]
pub enum BindingWatcherEvent {
  Close,
  Event,
  ReStart,
  Change,
}

impl From<BindingWatcherEvent> for rolldown_common::WatcherEvent {
  fn from(event: BindingWatcherEvent) -> Self {
    match event {
      BindingWatcherEvent::Close => rolldown_common::WatcherEvent::Close,
      BindingWatcherEvent::Event => rolldown_common::WatcherEvent::Event,
      BindingWatcherEvent::ReStart => rolldown_common::WatcherEvent::ReStart,
      BindingWatcherEvent::Change => rolldown_common::WatcherEvent::Change,
    }
  }
}
