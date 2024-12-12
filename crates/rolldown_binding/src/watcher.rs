use std::sync::Arc;
use std::time::Duration;

use napi_derive::napi;

use crate::bundler::{BindingBundlerOptions, Bundler};
use crate::types::binding_watcher_event::BindingWatcherEvent;

use napi::{tokio, Env};

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
    env: Env,
    options: Vec<BindingBundlerOptions>,
    notify_option: Option<BindingNotifyOption>,
  ) -> napi::Result<Self> {
    let bundlers = options
      .into_iter()
      .map(|option| Bundler::new(env, option).map(Bundler::into_inner))
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
    listener: MaybeAsyncJsCallback<BindingWatcherEvent, ()>,
  ) -> napi::Result<()> {
    let rx = Arc::clone(&self.inner.emitter().rx);
    let future = async move {
      let mut run = true;
      let rx = rx.lock().await;
      while run {
        match rx.recv() {
          Ok(event) => {
            if let rolldown_common::WatcherEvent::Close = &event {
              run = false;
            }
            tracing::debug!(name= "send event to js side", event = ?event);
            if let Err(e) = listener.await_call(BindingWatcherEvent::new(event)).await {
              eprintln!("watcher listener error: {e:?}");
            }
          }
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

    self.inner.start().await;
    Ok(())
  }
}
