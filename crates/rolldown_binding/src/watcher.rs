use std::sync::Arc;
use std::time::Duration;
use std::{error::Error as _, path::PathBuf};

use napi::{
  Env,
  bindgen_prelude::{FnArgs, PromiseRaw},
};
use napi_derive::napi;
use rolldown::BundlerConfig;
use rolldown_common::WatcherChangeKind;
use rolldown_error::BuildDiagnostic;
use rolldown_watcher::{
  CoordinatorCloseError, CoordinatorCloseFailure, WatchEvent, WatcherConfig, WatcherEventHandler,
  WatcherStartError,
};

use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_outputs::to_binding_error;
use crate::types::binding_watcher_event::BindingWatcherEvent;
use crate::types::error::{BindingError, NativeError};
use crate::types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt};
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};

fn watcher_close_native_error(message: impl Into<String>) -> BindingError {
  BindingError::NativeError(NativeError {
    kind: "WATCHER_CLOSE_ERROR".to_string(),
    message: message.into(),
    id: None,
    exporter: None,
    loc: None,
    pos: None,
  })
}

fn watcher_start_error_to_napi(error: WatcherStartError) -> napi::Error {
  napi::Error::from_reason(error.to_string())
}

fn coordinator_close_failure_to_binding_error(failure: &CoordinatorCloseFailure) -> BindingError {
  let source = failure.source();
  if let Some(error) = source.and_then(|source| {
    source.downcast_ref::<napi::Error>().or_else(|| {
      source
        .downcast_ref::<BuildDiagnostic>()
        .and_then(|diagnostic| diagnostic.downcast_napi_error().ok())
    })
  }) {
    return BindingError::from_napi_error(error);
  }

  if let Some(diagnostic) = source.and_then(|source| source.downcast_ref::<BuildDiagnostic>()) {
    return match to_binding_error(diagnostic, PathBuf::new()) {
      BindingError::JsError(error) => BindingError::JsError(error),
      BindingError::NativeError(mut error) => {
        error.message = failure.message().to_string();
        BindingError::NativeError(error)
      }
    };
  }

  watcher_close_native_error(failure.message())
}

#[napi(object, object_from_js = false)]
pub struct BindingWatcherCloseResult {
  pub errors: Vec<BindingError>,
  pub native_owned_close_identities: Vec<String>,
}

fn handle_watcher_close_result(
  result: anyhow::Result<()>,
  native_owned_close_identities: Vec<u64>,
) -> BindingWatcherCloseResult {
  let errors = match result {
    Ok(()) => Vec::new(),
    Err(error) => {
      error.chain().find_map(|cause| cause.downcast_ref::<CoordinatorCloseError>()).map_or_else(
        || vec![watcher_close_native_error(error.to_string())],
        |error| error.failures().iter().map(coordinator_close_failure_to_binding_error).collect(),
      )
    }
  };
  BindingWatcherCloseResult {
    errors,
    native_owned_close_identities: native_owned_close_identities
      .into_iter()
      .map(|identity| identity.to_string())
      .collect(),
  }
}

/// Split the flat per-output config list into config groups. `group_sizes[i]`
/// is the number of output configs created from input config `i`; the JS layer
/// builds the flat list in group order, so contiguous chunks reconstruct the
/// groups. Every output task of a group shares one native filesystem watcher.
fn split_configs_into_groups(
  configs: Vec<BundlerConfig>,
  group_sizes: &[u32],
) -> napi::Result<Vec<Vec<BundlerConfig>>> {
  if group_sizes.is_empty() {
    return Err(napi::Error::from_reason(
      "Watcher requires at least one config group, but groupSizes was empty",
    ));
  }
  if let Some(position) = group_sizes.iter().position(|&size| size == 0) {
    return Err(napi::Error::from_reason(format!(
      "Watcher config group {position} is empty; every group must contain at least one output config",
    )));
  }
  let total: usize = group_sizes.iter().map(|&size| size as usize).sum();
  if total != configs.len() {
    return Err(napi::Error::from_reason(format!(
      "Watcher groupSizes sum to {total}, but {} output configs were provided",
      configs.len(),
    )));
  }
  let mut configs = configs.into_iter();
  Ok(group_sizes.iter().map(|&size| configs.by_ref().take(size as usize).collect()).collect())
}

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
  async fn on_event(&self, event: WatchEvent) -> anyhow::Result<()> {
    let binding_event = BindingWatcherEvent::from_watch_event(event);
    self.listener.await_call(FnArgs { data: (binding_event,) }).await?;
    Ok(())
  }

  async fn on_change(&self, path: &str, kind: WatcherChangeKind) -> anyhow::Result<()> {
    let binding_event = BindingWatcherEvent::from_change(path.to_string(), kind.to_string());
    self.listener.await_call(FnArgs { data: (binding_event,) }).await?;
    Ok(())
  }

  async fn on_restart(&self) -> anyhow::Result<()> {
    let binding_event = BindingWatcherEvent::from_restart();
    self.listener.await_call(FnArgs { data: (binding_event,) }).await?;
    Ok(())
  }

  async fn on_close(&self) -> anyhow::Result<()> {
    let binding_event = BindingWatcherEvent::from_close();
    self.listener.await_call(FnArgs { data: (binding_event,) }).await?;
    Ok(())
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
    ts_args_type = "options: BindingBundlerOptions[], listener: (data: BindingWatcherEvent) => void, groupSizes: Array<number>"
  )]
  pub fn new(
    options: Vec<BindingBundlerOptions>,
    listener: MaybeAsyncJsCallback<FnArgs<(BindingWatcherEvent,)>>,
    group_sizes: Vec<u32>,
  ) -> napi::Result<Self> {
    let configs = options
      .into_iter()
      .map(create_bundler_config_from_binding_options)
      .collect::<Result<Vec<_>, _>>()?;

    // Shared-watcher settings (debounce, polling backend) span all groups, so
    // they are derived from the flat list before it is split into groups.
    let watcher_config = create_watcher_config(&configs);
    let groups = split_configs_into_groups(configs, &group_sizes)?;

    let handler = NapiWatcherEventHandler { listener: Arc::new(listener) };
    let inner =
      rolldown_watcher::Watcher::new(groups, handler, &watcher_config).map_err(|errs| {
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
  #[napi]
  pub fn close<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingWatcherCloseResult>> {
    let inner = Arc::clone(&self.inner);
    // Publish close before returning to JavaScript so an event listener cannot
    // return into a new build while its close task is still waiting to start.
    inner.publish_close();
    spawn_boxed_future(env, async move {
      let result = inner.close().await;
      Ok(handle_watcher_close_result(result, inner.native_owned_close_identities()))
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

  fn default_configs(count: usize) -> Vec<BundlerConfig> {
    (0..count).map(|_| BundlerConfig::new(BundlerOptions::default(), vec![])).collect()
  }

  #[test]
  fn group_split_reconstructs_contiguous_groups() {
    let groups = split_configs_into_groups(default_configs(3), &[2, 1])
      .expect("matching group sizes must split");
    assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
  }

  #[test]
  fn group_split_rejects_zero_groups() {
    let error = split_configs_into_groups(default_configs(1), &[])
      .expect_err("an empty group list must be rejected");
    assert_eq!(
      error.reason,
      "Watcher requires at least one config group, but groupSizes was empty"
    );
  }

  #[test]
  fn group_split_rejects_empty_group() {
    let error = split_configs_into_groups(default_configs(2), &[2, 0])
      .expect_err("a zero-size group must be rejected");
    assert_eq!(
      error.reason,
      "Watcher config group 1 is empty; every group must contain at least one output config"
    );
  }

  #[test]
  fn group_split_rejects_size_sum_mismatch() {
    let error = split_configs_into_groups(default_configs(3), &[2, 2])
      .expect_err("a group size sum mismatch must be rejected");
    assert_eq!(error.reason, "Watcher groupSizes sum to 4, but 3 output configs were provided");
  }
}
