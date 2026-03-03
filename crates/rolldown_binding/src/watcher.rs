use std::sync::Arc;
use std::time::Duration;

use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;
use rolldown_common::WatcherChangeKind;
use rolldown_watcher::{WatchEvent, WatcherConfig, WatcherEventHandler};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_watcher_event::BindingWatcherEvent;
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::create_bundler_config_from_binding_options::create_bundler_config_from_binding_options;

/// Bridges watcher events from Rust to JS via a `ThreadsafeFunction`.
struct NapiWatcherEventHandler {
  listener: Arc<MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>>,
}

impl WatcherEventHandler for NapiWatcherEventHandler {
  async fn on_event(&self, event: WatchEvent) {
    let binding_event = BindingWatcherEvent::from_watch_event(event);
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_event listener error: {e:?}");
    }
  }

  async fn on_change(&self, path: &str, kind: WatcherChangeKind) {
    let binding_event = BindingWatcherEvent::from_change(path.to_string(), kind.to_string());
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_change listener error: {e:?}");
    }
  }

  async fn on_restart(&self) {
    let binding_event = BindingWatcherEvent::from_restart();
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_restart listener error: {e:?}");
    }
  }

  async fn on_close(&self) {
    let binding_event = BindingWatcherEvent::from_close();
    if let Err(e) = self.listener.await_call(FnArgs { data: (binding_event,) }).await {
      eprintln!("watcher on_close listener error: {e:?}");
    }
  }
}

#[napi]
pub struct BindingWatcher {
  inner: rolldown_watcher::Watcher,
}

#[napi]
impl BindingWatcher {
  #[napi(
    constructor,
    ts_args_type = "options: BindingBundlerOptions[], listener: (data: BindingWatcherEvent) => void"
  )]
  pub fn new(
    options: Vec<BindingBundlerOptions>,
    listener: MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>,
  ) -> napi::Result<Self> {
    let configs = options
      .into_iter()
      .map(create_bundler_config_from_binding_options)
      .collect::<Result<Vec<_>, _>>()?;

    // Forward the largest build_delay from configs to the watcher's debounce.
    let build_delay =
      configs.iter().filter_map(|c| c.options.watch.as_ref().and_then(|w| w.build_delay)).max();

    // Extract use_polling / poll_interval / compare_contents_for_polling from the first config that specifies them.
    let use_polling = configs
      .iter()
      .find_map(|c| c.options.watch.as_ref().filter(|w| w.use_polling).map(|w| w.use_polling))
      .unwrap_or(false);
    let poll_interval =
      configs.iter().find_map(|c| c.options.watch.as_ref().and_then(|w| w.poll_interval));
    let compare_contents_for_polling = configs
      .iter()
      .find_map(|c| {
        c.options
          .watch
          .as_ref()
          .filter(|w| w.compare_contents_for_polling)
          .map(|w| w.compare_contents_for_polling)
      })
      .unwrap_or(false);

    let watcher_config = WatcherConfig {
      debounce: build_delay.map(|ms| Duration::from_millis(u64::from(ms))),
      use_polling,
      poll_interval,
      compare_contents_for_polling,
    };

    let handler = NapiWatcherEventHandler { listener: Arc::new(listener) };
    let inner =
      rolldown_watcher::Watcher::new(configs, handler, &watcher_config).map_err(|errs| {
        napi::Error::new(
          napi::Status::GenericFailure,
          errs.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
    Ok(Self { inner })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn run(&self) -> napi::Result<()> {
    self.inner.run();
    Ok(())
  }

  /// Gives consumers a reliable way to await the watcher's completion.
  /// The Node.js layer relies on the pending Promise to keep the process from exiting.
  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn wait_for_close(&self) -> napi::Result<()> {
    self.inner.wait_for_close().await;
    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    self
      .inner
      .close()
      .await
      .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
  }
}
