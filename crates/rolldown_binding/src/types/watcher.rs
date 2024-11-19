use std::sync::Arc;

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
    ts_args_type = "event: BindingWatcherEvent, listener: (data: BindingWatcherEventData) => void"
  )]
  pub fn on(
    &self,
    event: BindingWatcherEvent,
    listener: MaybeAsyncJsCallback<BindingWatcherEventData, ()>,
  ) -> napi::Result<()> {
    self.inner.emitter.on(
      event.into(),
      Box::new(move |data| {
        let listener = Arc::clone(&listener);
        Box::pin(async move {
          listener.await_call(BindingWatcherEventData::new(data)).await.map_err(anyhow::Error::from)
        })
      }),
    );
    Ok(())
  }

  #[napi]
  pub async fn start(&self) -> napi::Result<()> {
    self.inner.start().await;
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

#[napi]
pub struct BindingWatcherEventData {
  inner: Arc<rolldown_common::WatcherEventData>,
}

#[napi]
impl BindingWatcherEventData {
  pub fn new(inner: Arc<rolldown_common::WatcherEventData>) -> Self {
    Self { inner }
  }

  #[napi]
  pub fn watch_change_data(&self) -> BindingWatcherChangeData {
    if let rolldown_common::WatcherEventData::WatcherChange(data) = &*self.inner {
      BindingWatcherChangeData { path: data.path.to_string(), kind: data.kind.to_string() }
    } else {
      unreachable!("Expected WatcherEventData::Change")
    }
  }

  #[napi]
  pub fn bundle_end_data(&self) -> BindingBundleEndEventData {
    if let rolldown_common::WatcherEventData::BundleEvent(
      rolldown_common::BundleEventKind::BundleEnd(data),
    ) = &*self.inner
    {
      BindingBundleEndEventData { output: data.output.to_string(), duration: data.duration }
    } else {
      unreachable!("Expected WatcherEventData::BundleEvent(BundleEventKind::BundleEnd)")
    }
  }

  #[napi]
  pub fn bundle_event_kind(&self) -> String {
    if let rolldown_common::WatcherEventData::BundleEvent(kind) = &*self.inner {
      kind.to_string()
    } else {
      unreachable!("Expected WatcherEventData::BundleEvent")
    }
  }

  #[napi]
  pub fn error(&self) -> String {
    if let rolldown_common::WatcherEventData::BundleEvent(
      rolldown_common::BundleEventKind::Error(err),
    ) = &*self.inner
    {
      err.to_string()
    } else {
      unreachable!("Expected WatcherEventData::BundleEvent(BundleEventKind::Error)")
    }
  }
}

#[napi]
pub struct BindingWatcherChangeData {
  pub path: String,
  pub kind: String,
}

#[napi]
pub struct BindingBundleEndEventData {
  pub output: String,
  pub duration: u32,
}
