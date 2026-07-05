use napi_derive::napi;
use rolldown_dev::{
  BundleState, DevCallbackError, DevCallbackFuture, OnAdditionalAssetsCallback,
  OnHmrUpdatesCallback, OnOutputCallback,
};
use rolldown_error::BatchedBuildDiagnostic;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use crate::binding_dev_options::BindingDevOptions;
use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_client_hmr_update::BindingClientHmrUpdate;
use crate::types::binding_error_stage::BindingErrorStage;
use crate::types::binding_outputs::{BindingOutputs, to_binding_error};
use crate::types::error::{BindingErrors, BindingResult};
use crate::types::js_callback::MaybeAsyncJsCallbackExt;
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};
use futures::channel::oneshot;
use napi::bindgen_prelude::{FnArgs, PromiseRaw};
use napi::{Either, Env};

type BindingDevEngineCloseOutcome = Result<(), Arc<BatchedBuildDiagnostic>>;

fn dev_engine_close_error(error: &BatchedBuildDiagnostic) -> napi::Error {
  if let Some(error) = error.iter().find_map(|diagnostic| diagnostic.downcast_napi_error().ok()) {
    return error.try_clone().unwrap_or_else(|clone_error| clone_error);
  }
  napi::Error::from_reason(format!("Failed to close dev engine: {error:#}"))
}

fn dev_engine_operation_error(
  context: &'static str,
  error: &BatchedBuildDiagnostic,
) -> napi::Error {
  if let Some(error) = error.iter().find_map(|diagnostic| diagnostic.downcast_napi_error().ok()) {
    return error.try_clone().unwrap_or_else(|clone_error| clone_error);
  }
  napi::Error::from_reason(format!("{context}: {error:#}"))
}

fn dev_engine_closed_error() -> napi::Error {
  napi::Error::from_reason("Dev engine is closed")
}

// See internal-docs/dev-engine/implementation.md sections 15-16.
struct BindingDevEngineLifecycleState {
  closing: bool,
  active_operations: usize,
  operations_drained: Vec<oneshot::Sender<()>>,
  close_outcome: Option<BindingDevEngineCloseOutcome>,
}

struct BindingDevEngineLifecycle {
  state: StdMutex<BindingDevEngineLifecycleState>,
}

enum BeginBindingDevEngineClose {
  Close(Option<oneshot::Receiver<()>>),
  Finished(BindingDevEngineCloseOutcome),
}

impl BindingDevEngineLifecycle {
  fn new() -> Self {
    Self {
      state: StdMutex::new(BindingDevEngineLifecycleState {
        closing: false,
        active_operations: 0,
        operations_drained: Vec::new(),
        close_outcome: None,
      }),
    }
  }

  fn begin_operation(self: &Arc<Self>) -> Option<BindingDevEngineOperationGuard> {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.closing {
      return None;
    }
    state.active_operations += 1;
    Some(BindingDevEngineOperationGuard { lifecycle: Arc::clone(self) })
  }

  fn begin_close(&self) -> BeginBindingDevEngineClose {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(outcome) = state.close_outcome.clone() {
      return BeginBindingDevEngineClose::Finished(outcome);
    }
    state.closing = true;
    let operations_drained = if state.active_operations == 0 {
      None
    } else {
      let (sender, receiver) = oneshot::channel();
      state.operations_drained.push(sender);
      Some(receiver)
    };
    BeginBindingDevEngineClose::Close(operations_drained)
  }

  fn finish_close(&self, outcome: BindingDevEngineCloseOutcome) {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).close_outcome =
      Some(outcome);
  }
}

struct BindingDevEngineOperationGuard {
  lifecycle: Arc<BindingDevEngineLifecycle>,
}

impl Drop for BindingDevEngineOperationGuard {
  fn drop(&mut self) {
    let mut state = self.lifecycle.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    state.active_operations -= 1;
    if state.active_operations == 0 && state.closing {
      for operations_drained in std::mem::take(&mut state.operations_drained) {
        let _ = operations_drained.send(());
      }
    }
  }
}

#[napi]
pub struct BindingDevEngine {
  inner: Arc<rolldown_dev::DevEngine>,
  lifecycle: Arc<BindingDevEngineLifecycle>,
  cwd: Arc<Path>,
  _session_id: Arc<str>,
  _session: rolldown_devtools::Session,
}

#[napi]
impl BindingDevEngine {
  #[napi(constructor)]
  pub fn new(
    _env: Env,
    options: BindingBundlerOptions,
    dev_options: Option<BindingDevOptions>,
  ) -> napi::Result<Self> {
    let session_id = rolldown_devtools::generate_session_id();
    let session = rolldown_devtools::Session::dummy();

    let on_hmr_updates_callback = dev_options.as_ref().and_then(|opts| opts.on_hmr_updates.clone());
    let on_output_callback = dev_options.as_ref().and_then(|opts| opts.on_output.clone());
    let on_additional_assets_callback =
      dev_options.as_ref().and_then(|opts| opts.on_additional_assets.clone());

    let cwd: Arc<Path> = Arc::from(PathBuf::from(options.input_options.cwd.clone()));

    let rebuild_strategy =
      dev_options.as_ref().and_then(|opts| opts.rebuild_strategy).map(Into::into);
    // Take ownership of watch so we can consume Vec fields (include/exclude).
    let watch_options = dev_options.and_then(|opts| opts.watch);
    let skip_write = watch_options.as_ref().and_then(|watch| watch.skip_write);
    let use_polling = watch_options.as_ref().and_then(|watch| watch.use_polling);
    let poll_interval = watch_options.as_ref().and_then(|watch| watch.poll_interval);
    let use_debounce = watch_options.as_ref().and_then(|watch| watch.use_debounce);
    let debounce_duration = watch_options.as_ref().and_then(|watch| watch.debounce_duration);
    let compare_contents_for_polling =
      watch_options.as_ref().and_then(|watch| watch.compare_contents_for_polling);
    let debounce_tick_rate = watch_options.as_ref().and_then(|watch| watch.debounce_tick_rate);
    let (watch_include, watch_exclude) = watch_options
      .map(|watch| {
        let include = watch
          .include
          .map(crate::types::binding_string_or_regex::bindingify_string_or_regex_array);
        let exclude = watch
          .exclude
          .map(crate::types::binding_string_or_regex::bindingify_string_or_regex_array);
        (include, exclude)
      })
      .unwrap_or((None, None));

    // Create bundler config
    let bundler_config = create_bundler_config_from_binding_options(options)?;

    // If callback is provided, wrap it to convert BuildResult<(Vec<ClientHmrUpdate>, Vec<String>)> to BindingResult<(Vec<BindingClientHmrUpdate>, Vec<String>)>
    let on_hmr_updates = on_hmr_updates_callback.map(|js_callback| {
      let cwd = Arc::<Path>::clone(&cwd);
      Arc::new(
        move |result: rolldown_error::BuildResult<(
          Vec<rolldown_common::ClientHmrUpdate>,
          Vec<String>,
        )>|
              -> DevCallbackFuture {
          let js_callback = Arc::clone(&js_callback);
          let cwd = Arc::<Path>::clone(&cwd);
          Box::pin(async move {
            let binding_result: BindingResult<(Vec<BindingClientHmrUpdate>, Vec<String>)> =
              match result {
                Ok((updates, changed_files)) => {
                  let binding_updates: Vec<BindingClientHmrUpdate> =
                    updates.into_iter().map(BindingClientHmrUpdate::from).collect();
                  Either::B((binding_updates, changed_files))
                }
                Err(errors) => {
                  let binding_errors: Vec<_> = errors
                    .iter()
                    .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
                    .collect();
                  Either::A(BindingErrors::new(binding_errors))
                }
              };
            js_callback
              .await_call(FnArgs { data: (binding_result,) })
              .await
              .map_err(|error| Arc::new(error) as DevCallbackError)
          })
        },
      ) as OnHmrUpdatesCallback
    });

    // If callback is provided, wrap it to convert BuildResult<BundleOutput> to BindingResult<BindingOutputs>
    let on_output = on_output_callback.map(|js_callback| {
      let cwd = Arc::<Path>::clone(&cwd);
      Arc::new(
        move |result: rolldown_error::BuildResult<rolldown::BundleOutput>| -> DevCallbackFuture {
          let js_callback = Arc::clone(&js_callback);
          let cwd = Arc::<Path>::clone(&cwd);
          Box::pin(async move {
            let binding_result: BindingResult<BindingOutputs> = match result {
              Ok(bundle_output) => Either::B(BindingOutputs::from(bundle_output.assets)),
              Err(errors) => {
                let binding_errors: Vec<_> = errors
                  .iter()
                  .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
                  .collect();
                Either::A(BindingErrors::new(binding_errors))
              }
            };
            js_callback
              .await_call(FnArgs { data: (binding_result,) })
              .await
              .map_err(|error| Arc::new(error) as DevCallbackError)
          })
        },
      ) as OnOutputCallback
    });

    // Assets emitted during an HMR patch / lazy compile (these never go through
    // `on_output`). Forward the assets; warnings stay Rust-side, as in `on_output`.
    let on_additional_assets = on_additional_assets_callback.map(|js_callback| {
      Arc::new(move |output: rolldown::BundleOutput| -> DevCallbackFuture {
        let js_callback = Arc::clone(&js_callback);
        Box::pin(async move {
          let binding_outputs = BindingOutputs::from(output.assets);
          js_callback
            .await_call(FnArgs { data: (binding_outputs,) })
            .await
            .map_err(|error| Arc::new(error) as DevCallbackError)
        })
      }) as OnAdditionalAssetsCallback
    });

    let dev_watch_options = if skip_write.is_some()
      || use_polling.is_some()
      || poll_interval.is_some()
      || use_debounce.is_some()
      || debounce_duration.is_some()
      || compare_contents_for_polling.is_some()
      || debounce_tick_rate.is_some()
      || watch_include.is_some()
      || watch_exclude.is_some()
    {
      Some(rolldown_dev::DevWatchOptions {
        disable_watcher: None,
        skip_write,
        use_polling,
        poll_interval: poll_interval.map(u64::from),
        use_debounce,
        debounce_duration: debounce_duration.map(u64::from),
        compare_contents_for_polling,
        debounce_tick_rate: debounce_tick_rate.map(u64::from),
        include: watch_include,
        exclude: watch_exclude,
      })
    } else {
      None
    };

    let rolldown_dev_options = rolldown_dev::DevOptions {
      on_hmr_updates,
      on_output,
      on_additional_assets,
      rebuild_strategy,
      watch: dev_watch_options,
    };

    let inner = rolldown_dev::DevEngine::new(bundler_config, rolldown_dev_options)
      .map_err(|e| napi::Error::from_reason(format!("Fail to create dev engine: {e:#?}")))?;

    Ok(Self {
      inner: Arc::new(inner),
      lifecycle: Arc::new(BindingDevEngineLifecycle::new()),
      cwd,
      _session_id: session_id,
      _session: session,
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn run<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      let result = inner
        .run()
        .await
        .map_err(|error| dev_engine_operation_error("Failed to run dev engine", &error));
      drop(operation);
      result
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn ensure_current_build_finish<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::resolve(env, ());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      let result = inner.wait_for_ongoing_bundle().await.map_err(|error| {
        dev_engine_operation_error("Failed to ensure current build finish", &error)
      });
      drop(operation);
      result
    })
  }

  #[napi]
  pub fn get_bundle_state<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingBundleState>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      let result = inner
        .get_bundle_state()
        .await
        .map(Into::into)
        .map_err(|_e| napi::Error::from_reason("Failed to get bundle state"));
      drop(operation);
      result
    })
  }

  #[napi]
  pub fn ensure_latest_build_output<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      let result = match inner.ensure_latest_bundle_output().await {
        Ok(()) => Ok(Either::B(())),
        Err(errors) => {
          let binding_errors: Vec<_> = errors
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
            .collect();
          Ok(Either::A(BindingErrors::new(binding_errors)))
        }
      };
      drop(operation);
      result
    })
  }

  #[napi]
  pub fn trigger_full_build(&self) -> napi::Result<()> {
    let Some(_operation) = self.lifecycle.begin_operation() else {
      return Err(dev_engine_closed_error());
    };
    self
      .inner
      .trigger_full_build()
      .map_err(|error| napi::Error::from_reason(format!("Failed to trigger full build: {error:#}")))
  }

  #[napi]
  pub fn invalidate<'env>(
    &self,
    env: &'env Env,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<Vec<BindingClientHmrUpdate>>>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      let result = match inner.invalidate(caller, first_invalidated_by).await {
        Ok(updates) => {
          let binding_updates =
            updates.into_iter().map(BindingClientHmrUpdate::from).collect::<Vec<_>>();
          Ok(Either::B(binding_updates))
        }
        Err(errors) => {
          let binding_errors: Vec<_> = errors
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
            .collect();
          Ok(Either::A(BindingErrors::new(binding_errors)))
        }
      };
      drop(operation);
      result
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn register_modules<'env>(
    &self,
    env: &'env Env,
    client_id: String,
    modules: Vec<String>,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.clients.lock().await.entry(client_id).or_default().executed_modules.extend(modules);
      drop(operation);
      Ok(())
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn remove_client<'env>(
    &self,
    env: &'env Env,
    client_id: String,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.clients.lock().await.remove(&client_id);
      drop(operation);
      Ok(())
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let close = match self.lifecycle.begin_close() {
      BeginBindingDevEngineClose::Finished(outcome) => {
        return match outcome {
          Ok(()) => PromiseRaw::resolve(env, ()),
          Err(error) => PromiseRaw::reject(env, dev_engine_close_error(error.as_ref())),
        };
      }
      close @ BeginBindingDevEngineClose::Close(_) => close,
    };

    let inner = Arc::clone(&self.inner);
    let lifecycle = Arc::clone(&self.lifecycle);
    spawn_boxed_future(env, async move {
      let outcome = match close {
        BeginBindingDevEngineClose::Close(operations_drained) => {
          let outcome = inner.close().await;
          if let Some(operations_drained) = operations_drained {
            let _ = operations_drained.await;
          }
          lifecycle.finish_close(outcome.clone());
          outcome
        }
        BeginBindingDevEngineClose::Finished(outcome) => outcome,
      };
      outcome.map_err(|error| dev_engine_close_error(error.as_ref()))
    })
  }

  /// Compile a lazy entry module and return HMR-style patch code.
  ///
  /// This is called when a dynamically imported module is first requested at runtime.
  /// The module was previously stubbed with a proxy, and now we need to compile the
  /// actual module and its dependencies.
  #[napi]
  pub fn compile_entry<'env>(
    &self,
    env: &'env Env,
    module_id: String,
    client_id: String,
  ) -> napi::Result<PromiseRaw<'env, String>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      let result = inner
        .compile_lazy_entry(module_id, client_id)
        .await
        .map_err(|error| dev_engine_operation_error("Failed to compile lazy entry", &error));
      drop(operation);
      result
    })
  }
}

#[napi(object)]
pub struct BindingBundleState {
  pub last_build_errored: bool,
  /// The stage of the last incremental failure, when `last_build_errored`
  /// is true and the engine is in an incremental-failure state. Absent on
  /// success and for an initial full-build failure (use
  /// `last_build_errored` to detect that). The consumer can force a full
  /// rebuild on the next page load when this is `Hmr`. See
  /// `internal-docs/dev-engine/implementation.md` §12.
  pub last_error_stage: Option<BindingErrorStage>,
  pub has_stale_output: bool,
}

impl From<BundleState> for BindingBundleState {
  fn from(state: BundleState) -> Self {
    Self {
      last_build_errored: state.last_build_errored,
      last_error_stage: state.last_error_stage.map(Into::into),
      has_stale_output: state.has_stale_output,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn lifecycle_close_waits_for_active_operations_and_replays_outcome() {
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());
    let first_operation = lifecycle.begin_operation().expect("engine should start open");
    let second_operation = lifecycle.begin_operation().expect("engine should start open");

    let BeginBindingDevEngineClose::Close(Some(mut first_close)) = lifecycle.begin_close() else {
      panic!("close should wait for active operations");
    };
    let BeginBindingDevEngineClose::Close(Some(mut concurrent_close)) = lifecycle.begin_close()
    else {
      panic!("concurrent close should share the operation barrier");
    };
    assert!(lifecycle.begin_operation().is_none(), "close must reject new operations");
    assert_eq!(first_close.try_recv().expect("sender should remain live"), None);
    assert_eq!(concurrent_close.try_recv().expect("sender should remain live"), None);

    drop(first_operation);
    assert_eq!(first_close.try_recv().expect("sender should remain live"), None);
    drop(second_operation);
    assert_eq!(first_close.try_recv().expect("barrier should resolve"), Some(()));
    assert_eq!(concurrent_close.try_recv().expect("barrier should resolve"), Some(()));

    lifecycle.finish_close(Ok(()));
    assert!(matches!(lifecycle.begin_close(), BeginBindingDevEngineClose::Finished(Ok(()))));
  }
}
