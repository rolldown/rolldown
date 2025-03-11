use std::sync::Arc;
use std::time::Duration;

use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;

use crate::bundler::{BindingBundlerOptions, Bundler};
use crate::types::binding_watcher_event::BindingWatcherEvent;

use napi::Env;

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
  inner: Arc<rolldown::Watcher>,
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

    Ok(Self { inner: Arc::new(rolldown::Watcher::new(bundlers, notify_option.map(Into::into))?) })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    handle_result(self.inner.close().await)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_args_type = "listener: (data: BindingWatcherEvent) => void")]
  pub fn start(
    &self,
    listener: MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>, ()>,
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

    #[cfg(target_family = "wasm")]
    {
      std::thread::spawn(|| {
        let rt = napi::tokio::runtime::Builder::new_current_thread().build();
        match rt {
          Ok(rt) => rt.block_on(future),
          Err(e) => tracing::error!("create runtime error: {e:?}"),
        }
      });
    }
    #[cfg(not(target_family = "wasm"))]
    {
      napi::tokio::spawn(future);
    }

    #[cfg(target_family = "wasm")]
    {
      let inner = Arc::clone(&self.inner);
      std::thread::spawn(move || {
        let rt = napi::tokio::runtime::Builder::new_current_thread().build();
        match rt {
          Ok(rt) => rt.block_on(inner.start()),
          Err(e) => tracing::error!("create runtime error: {e:?}"),
        }
      });
    }

    #[cfg(not(target_family = "wasm"))]
    {
      let inner = Arc::clone(&self.inner);
      napi::tokio::spawn(async move {
        inner.start().await;
      });
    }
    Ok(())
  }
}
