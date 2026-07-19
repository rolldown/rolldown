#![expect(clippy::print_stderr)]

use std::sync::Arc;
use std::time::Duration;

use napi::{
  Env,
  bindgen_prelude::{FnArgs, PromiseRaw},
};
use napi_derive::napi;
use rolldown::BundlerConfig;
use rolldown_common::WatcherChangeKind;
use rolldown_watcher::{WatchEvent, WatcherConfig, WatcherEventHandler};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_watcher_event::BindingWatcherEvent;
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};

fn create_watcher_config(configs: &[BundlerConfig]) -> WatcherConfig {
  // Rollup applies the maximum buildDelay across enabled configs. The file
  // watcher itself is shared in Rolldown, so use the first config that
  // explicitly selects backend/debouncer behavior for the remaining fields.
  let debounce = configs
    .iter()
    .filter_map(|config| config.options.watch.as_ref()?.build_delay)
    .max()
    .map(|ms| Duration::from_millis(u64::from(ms)));
  let watch = configs
    .iter()
    .filter_map(|config| config.options.watch.as_ref())
    .find(|watch| {
      watch.use_polling
        || watch.poll_interval.is_some()
        || watch.compare_contents_for_polling
        || watch.use_debounce
        || watch.debounce_delay.is_some()
        || watch.debounce_tick_rate.is_some()
    })
    .or_else(|| configs.first().and_then(|config| config.options.watch.as_ref()));
  WatcherConfig {
    debounce,
    use_polling: watch.is_some_and(|watch| watch.use_polling),
    poll_interval: watch.and_then(|watch| watch.poll_interval),
    compare_contents_for_polling: watch.is_some_and(|watch| watch.compare_contents_for_polling),
    use_debounce: watch.is_some_and(|watch| watch.use_debounce),
    debounce_delay: watch.and_then(|watch| watch.debounce_delay),
    debounce_tick_rate: watch.and_then(|watch| watch.debounce_tick_rate),
  }
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

    let watcher_config = create_watcher_config(&configs);

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
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.run();
      Ok(())
    })
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
    spawn_boxed_future(env, async move {
      inner.close().await.map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use rolldown::BundlerOptions;
  use rolldown_common::WatchOption;

  #[test]
  fn watcher_config_uses_max_build_delay_and_first_explicit_backend_config() {
    let configs = vec![
      BundlerConfig::new(
        BundlerOptions {
          watch: Some(WatchOption { build_delay: Some(50), ..Default::default() }),
          ..Default::default()
        },
        vec![],
      ),
      BundlerConfig::new(
        BundlerOptions {
          watch: Some(WatchOption {
            build_delay: Some(250),
            use_polling: true,
            poll_interval: Some(75),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![],
      ),
    ];

    let config = create_watcher_config(&configs);
    assert_eq!(config.debounce, Some(Duration::from_millis(250)));
    assert!(config.use_polling);
    assert_eq!(config.poll_interval, Some(75));
  }
}
