#![expect(clippy::print_stderr)]

use std::sync::Arc;
use std::time::Duration;

use napi::{
  Env,
  bindgen_prelude::{FnArgs, PromiseRaw},
};
use napi_derive::napi;
use rolldown_common::WatcherChangeKind;
use rolldown_watcher::{WatchEvent, WatcherConfig, WatcherEventHandler, WatcherStartError};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_watcher_event::BindingWatcherEvent;
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};

fn watcher_start_error_to_napi(error: WatcherStartError) -> napi::Error {
  napi::Error::from_reason(error.to_string())
}

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
  inner: Arc<rolldown_watcher::Watcher>,
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

    // Extract watcher config from the first config's watch options.
    let watch = configs.first().and_then(|c| c.options.watch.as_ref());
    let watcher_config = WatcherConfig {
      debounce: watch.and_then(|w| w.build_delay).map(|ms| Duration::from_millis(u64::from(ms))),
      use_polling: watch.is_some_and(|w| w.use_polling),
      poll_interval: watch.and_then(|w| w.poll_interval),
      compare_contents_for_polling: watch.is_some_and(|w| w.compare_contents_for_polling),
      use_debounce: watch.is_some_and(|w| w.use_debounce),
      debounce_delay: watch.and_then(|w| w.debounce_delay),
      debounce_tick_rate: watch.and_then(|w| w.debounce_tick_rate),
    };

    let handler = NapiWatcherEventHandler { listener: Arc::new(listener) };
    let inner =
      rolldown_watcher::Watcher::new(configs, handler, &watcher_config).map_err(|errs| {
        napi::Error::new(
          napi::Status::GenericFailure,
          errs.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
    Ok(Self { inner: Arc::new(inner) })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_return_type = "Promise<void>")]
  pub fn run<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    // Shared-runtime submission is thread-safe. Attempt it before entering a
    // N-API future so a stopped runtime can still return a rejected Promise
    // while preserving the coordinator for a later explicit retry.
    match self.inner.run().map_err(watcher_start_error_to_napi) {
      Ok(()) => PromiseRaw::resolve(env, ()),
      Err(error) => PromiseRaw::reject(env, error),
    }
  }

  /// Gives consumers a reliable way to await the watcher's completion.
  /// The Node.js layer relies on the pending Promise to keep the process from exiting.
  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_return_type = "Promise<void>")]
  pub fn wait_for_close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.wait_for_close().await;
      Ok(())
    })
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[napi(ts_return_type = "Promise<void>")]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    // Publish close before returning to JavaScript so an event listener cannot
    // return into a new build while its close task is still waiting to start.
    inner.publish_close();
    spawn_boxed_future(env, async move {
      inner.close().await.map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
    })
  }
}
