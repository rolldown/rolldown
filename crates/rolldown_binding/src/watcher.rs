use std::sync::Arc;

use napi_derive::napi;

use crate::bundler::{BindingBundlerOption, Bundler};
use crate::types::binding_watcher_event::BindingWatcherEvent;

use napi::{tokio, Env};

use crate::utils::handle_result;

use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};

#[napi]
pub struct BindingWatcher {
  inner: rolldown::Watcher,
}

#[napi]
impl BindingWatcher {
  #[napi(constructor)]
  pub fn new(env: Env, options: Vec<BindingBundlerOption>) -> napi::Result<Self> {
    let bundlers = options
      .into_iter()
      .map(|option| Bundler::new(env, option).map(Bundler::into_inner))
      .collect::<Result<Vec<_>, _>>()?;

    Ok(Self { inner: rolldown::Watcher::new(bundlers)? })
  }

  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    handle_result(self.inner.close().await)
  }

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
