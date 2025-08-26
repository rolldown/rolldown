use std::sync::Arc;
use std::time::Duration;

use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};
use crate::types::binding_watcher_event::BindingWatcherEvent;

use crate::utils::handle_result;

use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingNotifyOption {
  pub poll_interval: Option<u32>,
  pub compare_contents: Option<bool>,
}

impl From<BindingNotifyOption> for rolldown_common::NotifyOption {
  #[allow(clippy::cast_lossless)]
  fn from(value: BindingNotifyOption) -> Self {
    Self {
      poll_interval: value.poll_interval.map(|m| Duration::from_millis(m as u64)),
      compare_contents: value.compare_contents.unwrap_or_default(),
    }
  }
}

#[napi]
pub struct BindingWatcher {
  inner: rolldown::Watcher,
}

#[napi]
impl BindingWatcher {
  #[napi(constructor)]
  pub fn new(
    options: Vec<BindingBundlerOptions>,
    notify_option: Option<BindingNotifyOption>,
  ) -> napi::Result<Self> {
    let bundlers = options
      .into_iter()
      .map(|option| {
        // TODO(hyf0): support emit debug data for builtin watch
        BindingBundlerImpl::new(option, rolldown_debug::Session::dummy(), 0)
          .map(BindingBundlerImpl::into_inner)
      })
      .collect::<Result<Vec<_>, _>>()?;

    Ok(Self { inner: rolldown::Watcher::new(bundlers, notify_option.map(Into::into))? })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    handle_result(self.inner.close().await)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_args_type = "listener: (data: BindingWatcherEvent) => void")]
  pub async fn start(
    &self,
    listener: MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>,
  ) -> napi::Result<()> {
    let rx = Arc::clone(&self.inner.emitter().rx);
    let future = async move {
      let mut run = true;
      let rx = rx.lock().await;
      while run {
        match rx.recv() {
          Ok(event) => {
            if let rolldown::WatcherEvent::Close = &event {
              run = false;
            }
            tracing::debug!(name= "send event to js side", event = ?event);
            if let Err(e) =
              listener.await_call(FnArgs { data: (BindingWatcherEvent::new(event),) }).await
            {
              eprintln!("watcher listener error: {e:?}");
            }
          }
          Err(e) => {
            eprintln!("watcher receiver error: {e:?}");
          }
        }
      }
    };
    napi::tokio::spawn(future);
    self.inner.start().await;
    Ok(())
  }
}
