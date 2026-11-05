#![expect(clippy::print_stderr)]

use std::sync::Arc;
use std::time::Duration;

use napi::{
  Env,
  bindgen_prelude::{FnArgs, PromiseRaw},
};
use napi_derive::napi;
use rolldown::BundlerConfig;
use rolldown_common::{WatcherChangeKind, WatcherFileWatcherOptions};
use rolldown_watcher::{WatchEvent, WatcherConfig, WatcherEventHandler};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_watcher_event::BindingWatcherEvent;
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};

/// Whether a config sets any fs-watcher backend/debouncer field.
fn sets_watcher_backend(watcher: &WatcherFileWatcherOptions) -> bool {
  watcher.use_polling.is_some()
    || watcher.poll_interval.is_some()
    || watcher.compare_contents_for_polling.is_some()
    || watcher.use_debounce.is_some()
    || watcher.debounce_delay.is_some()
    || watcher.debounce_tick_rate.is_some()
}

/// Folds every config's watch options into the one `WatcherConfig` shared by all tasks.
// See internal-docs/watch-mode/implementation.md ("Multi-Config Watcher Options").
fn create_watcher_config(configs: &[BundlerConfig]) -> WatcherConfig {
  // Rollup waits for the largest `buildDelay` of all configs.
  let debounce = configs
    .iter()
    .filter_map(|config| config.options.watch.as_ref()?.build_delay)
    .max()
    .map(|ms| Duration::from_millis(u64::from(ms)));
  // Backend fields cannot be merged; the first config that sets one wins (`MULTIPLE_WATCHER_OPTION`).
  let watcher = configs
    .iter()
    .filter_map(|config| config.options.watch.as_ref()?.watcher.as_ref())
    .find(|watcher| sets_watcher_backend(watcher));
  WatcherConfig {
    debounce,
    use_polling: watcher.and_then(|w| w.use_polling).unwrap_or_default(),
    poll_interval: watcher.and_then(|w| w.poll_interval),
    compare_contents_for_polling: watcher
      .and_then(|w| w.compare_contents_for_polling)
      .unwrap_or_default(),
    use_debounce: watcher.and_then(|w| w.use_debounce).unwrap_or_default(),
    debounce_delay: watcher.and_then(|w| w.debounce_delay),
    debounce_tick_rate: watcher.and_then(|w| w.debounce_tick_rate),
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

  fn config(watcher: Option<WatcherFileWatcherOptions>, build_delay: Option<u32>) -> BundlerConfig {
    let watch = WatchOption { build_delay, watcher, ..Default::default() };
    BundlerConfig::new(BundlerOptions { watch: Some(watch), ..Default::default() }, vec![])
  }

  #[test]
  fn watcher_config_uses_max_build_delay_and_first_explicit_backend_config() {
    let configs = vec![
      config(Some(WatcherFileWatcherOptions::default()), Some(50)),
      config(
        Some(WatcherFileWatcherOptions {
          use_polling: Some(true),
          poll_interval: Some(75),
          ..Default::default()
        }),
        Some(250),
      ),
      config(
        Some(WatcherFileWatcherOptions { use_polling: Some(false), ..Default::default() }),
        None,
      ),
    ];
    let watcher_config = create_watcher_config(&configs);
    assert_eq!(watcher_config.debounce, Some(Duration::from_millis(250)));
    assert!(watcher_config.use_polling);
    assert_eq!(watcher_config.poll_interval, Some(75));

    // An explicit `usePolling: false` in an earlier config wins over a later `true`.
    let configs = vec![
      config(
        Some(WatcherFileWatcherOptions { use_polling: Some(false), ..Default::default() }),
        None,
      ),
      config(
        Some(WatcherFileWatcherOptions { use_polling: Some(true), ..Default::default() }),
        None,
      ),
    ];
    assert!(!create_watcher_config(&configs).use_polling);

    // A debouncer-only earlier config also selects the backend: a later `usePolling` is ignored.
    let configs = vec![
      config(
        Some(WatcherFileWatcherOptions {
          use_debounce: Some(true),
          debounce_delay: Some(10),
          debounce_tick_rate: Some(5),
          ..Default::default()
        }),
        None,
      ),
      config(
        Some(WatcherFileWatcherOptions { use_polling: Some(true), ..Default::default() }),
        None,
      ),
    ];
    let watcher_config = create_watcher_config(&configs);
    assert!(!watcher_config.use_polling);
    assert!(watcher_config.use_debounce);
    assert_eq!(watcher_config.debounce_delay, Some(10));
    assert_eq!(watcher_config.debounce_tick_rate, Some(5));
  }
}
