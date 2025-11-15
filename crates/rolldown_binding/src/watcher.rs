use std::sync::Arc;
use std::time::Duration;

use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_watcher_event::BindingWatcherEvent;

use crate::utils::handle_result;
use crate::utils::normalize_binding_options::normalize_binding_options;

use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingNotifyOption {
  pub poll_interval: Option<u32>,
  pub compare_contents: Option<bool>,
}

impl From<BindingNotifyOption> for rolldown_common::NotifyOption {
  #[expect(clippy::cast_lossless)]
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
    let options_and_plugins = options
      .into_iter()
      .map(|options| {
        // TODO(hyf0): support emit debug data for builtin watch
        let BindingBundlerOptions { input_options, output_options, parallel_plugins_registry } =
          options;

        #[cfg(not(target_family = "wasm"))]
        let worker_count = parallel_plugins_registry
          .as_ref()
          .map(|registry| registry.worker_count)
          .unwrap_or_default();
        #[cfg(not(target_family = "wasm"))]
        let parallel_plugins_map =
          parallel_plugins_registry.map(|registry| registry.take_plugin_values());

        #[cfg(not(target_family = "wasm"))]
        let worker_manager = if worker_count > 0 {
          use crate::worker_manager::WorkerManager;
          Some(WorkerManager::new(worker_count))
        } else {
          None
        };

        let normalized = normalize_binding_options(
          input_options,
          output_options,
          #[cfg(not(target_family = "wasm"))]
          parallel_plugins_map,
          #[cfg(not(target_family = "wasm"))]
          worker_manager,
        )?;

        Ok((normalized.bundler_options, normalized.plugins))
      })
      .collect::<Result<Vec<_>, napi::Error>>()?;

    let inner = rolldown::Watcher::new(options_and_plugins, notify_option.map(Into::into))
      .map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;

    Ok(Self { inner })
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
