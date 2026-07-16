use napi_derive::napi;
use rolldown_dev::{
  BundleState, DevCallbackError, DevCallbackFuture, OnAdditionalAssetsCallback,
  OnHmrUpdatesCallback, OnOutputCallback,
};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult};
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
  DetachedFutureSpawn,
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future, try_spawn_detached_future,
};
use futures::channel::oneshot;
use napi::bindgen_prelude::{FnArgs, PromiseRaw};
use napi::{Either, Env};

type BindingDevEngineCloseOutcome = Result<(), Arc<BatchedBuildDiagnostic>>;

fn dev_engine_binding_errors(error: &BatchedBuildDiagnostic, cwd: &Path) -> BindingErrors {
  BindingErrors::new(
    error.iter().map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf())).collect(),
  )
}

fn dev_engine_binding_result<T>(result: BuildResult<T>, cwd: &Path) -> BindingResult<T> {
  match result {
    Ok(value) => Either::B(value),
    Err(error) => Either::A(dev_engine_binding_errors(&error, cwd)),
  }
}

fn dev_engine_close_binding_result(
  outcome: BindingDevEngineCloseOutcome,
  cwd: &Path,
) -> BindingResult<()> {
  match outcome {
    Ok(()) => Either::B(()),
    Err(error) => Either::A(dev_engine_binding_errors(error.as_ref(), cwd)),
  }
}

fn dev_engine_closed_error() -> napi::Error {
  napi::Error::from_reason("Dev engine is closed")
}

// See internal-docs/dev-engine/implementation.md sections 15-16.
struct BindingDevEngineLifecycleState {
  closing: bool,
  active_operations: usize,
  active_callbacks: usize,
  operations_drained: Vec<oneshot::Sender<()>>,
  close_started: bool,
  close_finished: Vec<oneshot::Sender<BindingDevEngineCloseOutcome>>,
  close_outcome: Option<BindingDevEngineCloseOutcome>,
}

struct BindingDevEngineLifecycle {
  state: StdMutex<BindingDevEngineLifecycleState>,
}

enum BeginBindingDevEngineClose {
  Start { operations_drained: Option<oneshot::Receiver<()>>, acknowledge: bool },
  Wait(oneshot::Receiver<BindingDevEngineCloseOutcome>),
  Acknowledge,
  Finished(BindingDevEngineCloseOutcome),
}

impl BindingDevEngineLifecycle {
  fn new() -> Self {
    Self {
      state: StdMutex::new(BindingDevEngineLifecycleState {
        closing: false,
        active_operations: 0,
        active_callbacks: 0,
        operations_drained: Vec::new(),
        close_started: false,
        close_finished: Vec::new(),
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

  fn begin_callback(self: &Arc<Self>) -> BindingDevEngineCallbackGuard {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).active_callbacks += 1;
    BindingDevEngineCallbackGuard { lifecycle: Arc::clone(self) }
  }

  fn begin_close(&self) -> BeginBindingDevEngineClose {
    self.begin_close_with_callback_acknowledgement(true)
  }

  fn begin_terminal_close(&self) -> BeginBindingDevEngineClose {
    self.begin_close_with_callback_acknowledgement(false)
  }

  fn begin_close_with_callback_acknowledgement(
    &self,
    allow_callback_acknowledgement: bool,
  ) -> BeginBindingDevEngineClose {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(outcome) = state.close_outcome.clone() {
      return BeginBindingDevEngineClose::Finished(outcome);
    }
    state.closing = true;
    let acknowledge = allow_callback_acknowledgement && state.active_callbacks > 0;
    if state.close_started {
      if acknowledge {
        return BeginBindingDevEngineClose::Acknowledge;
      }
      let (sender, receiver) = oneshot::channel();
      state.close_finished.push(sender);
      return BeginBindingDevEngineClose::Wait(receiver);
    }
    state.close_started = true;
    let operations_drained = if state.active_operations == 0 {
      None
    } else {
      let (sender, receiver) = oneshot::channel();
      state.operations_drained.push(sender);
      Some(receiver)
    };
    BeginBindingDevEngineClose::Start { operations_drained, acknowledge }
  }

  fn finish_close(&self, outcome: BindingDevEngineCloseOutcome) {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    state.close_outcome = Some(outcome.clone());
    for close_finished in std::mem::take(&mut state.close_finished) {
      let _ = close_finished.send(outcome.clone());
    }
  }

  fn abort_close_start(&self) {
    let close_finished = {
      let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      if state.close_outcome.is_some() {
        return;
      }
      state.close_started = false;
      std::mem::take(&mut state.close_finished)
    };
    drop(close_finished);
  }
}

struct BindingDevEngineCloseExecutionGuard {
  lifecycle: Arc<BindingDevEngineLifecycle>,
  armed: bool,
}

impl BindingDevEngineCloseExecutionGuard {
  fn new(lifecycle: Arc<BindingDevEngineLifecycle>) -> Self {
    Self { lifecycle, armed: true }
  }

  fn finish(mut self, outcome: BindingDevEngineCloseOutcome) {
    self.lifecycle.finish_close(outcome);
    self.armed = false;
  }
}

impl Drop for BindingDevEngineCloseExecutionGuard {
  fn drop(&mut self) {
    if self.armed {
      self.lifecycle.abort_close_start();
    }
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

struct BindingDevEngineCallbackGuard {
  lifecycle: Arc<BindingDevEngineLifecycle>,
}

impl Drop for BindingDevEngineCallbackGuard {
  fn drop(&mut self) {
    self
      .lifecycle
      .state
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .active_callbacks -= 1;
  }
}

async fn execute_binding_dev_engine_close(
  inner: Arc<rolldown_dev::DevEngine>,
  close_execution: BindingDevEngineCloseExecutionGuard,
  operations_drained: Option<oneshot::Receiver<()>>,
) -> BindingDevEngineCloseOutcome {
  if let Some(operations_drained) = operations_drained {
    let _ = operations_drained.await;
  }
  let outcome = inner.close().await;
  close_execution.finish(outcome.clone());
  outcome
}

fn spawn_terminal_binding_dev_engine_close(
  env: &Env,
  inner: Arc<rolldown_dev::DevEngine>,
  lifecycle: Arc<BindingDevEngineLifecycle>,
  cwd: Arc<Path>,
  operations_drained: Option<oneshot::Receiver<()>>,
) -> napi::Result<PromiseRaw<'_, BindingResult<()>>> {
  let close_execution = BindingDevEngineCloseExecutionGuard::new(Arc::clone(&lifecycle));
  spawn_boxed_future(env, async move {
    let outcome =
      execute_binding_dev_engine_close(inner, close_execution, operations_drained).await;
    Ok(dev_engine_close_binding_result(outcome, cwd.as_ref()))
  })
}

fn wait_for_terminal_binding_dev_engine_close(
  env: &Env,
  cwd: Arc<Path>,
  close_finished: oneshot::Receiver<BindingDevEngineCloseOutcome>,
) -> napi::Result<PromiseRaw<'_, BindingResult<()>>> {
  spawn_boxed_future(env, async move {
    let outcome = close_finished.await.map_err(|_| {
      napi::Error::from_reason("Dev engine close task ended without publishing its result")
    })?;
    Ok(dev_engine_close_binding_result(outcome, cwd.as_ref()))
  })
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
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());

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
      let lifecycle = Arc::clone(&lifecycle);
      Arc::new(
        move |result: rolldown_error::BuildResult<(
          Vec<rolldown_common::ClientHmrUpdate>,
          Vec<String>,
        )>|
              -> DevCallbackFuture {
          let js_callback = Arc::clone(&js_callback);
          let cwd = Arc::<Path>::clone(&cwd);
          let lifecycle = Arc::clone(&lifecycle);
          Box::pin(async move {
            let _callback = lifecycle.begin_callback();
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
      let lifecycle = Arc::clone(&lifecycle);
      Arc::new(
        move |result: rolldown_error::BuildResult<rolldown::BundleOutput>| -> DevCallbackFuture {
          let js_callback = Arc::clone(&js_callback);
          let cwd = Arc::<Path>::clone(&cwd);
          let lifecycle = Arc::clone(&lifecycle);
          Box::pin(async move {
            let _callback = lifecycle.begin_callback();
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
      let lifecycle = Arc::clone(&lifecycle);
      Arc::new(move |output: rolldown::BundleOutput| -> DevCallbackFuture {
        let js_callback = Arc::clone(&js_callback);
        let lifecycle = Arc::clone(&lifecycle);
        Box::pin(async move {
          let _callback = lifecycle.begin_callback();
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

    Ok(Self { inner: Arc::new(inner), lifecycle, cwd, _session_id: session_id, _session: session })
  }

  #[napi]
  pub fn run<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      let result = dev_engine_binding_result(inner.run().await, cwd.as_ref());
      drop(operation);
      Ok(result)
    })
  }

  #[napi]
  pub fn ensure_current_build_finish<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::resolve(env, Either::B(()));
    };
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      let result = dev_engine_binding_result(inner.wait_for_ongoing_bundle().await, cwd.as_ref());
      drop(operation);
      Ok(result)
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

  /// Client-connect signal (the clientId hello): creates the per-client session
  /// with an empty ship map. Reconnects arrive as fresh clientIds.
  #[napi(ts_return_type = "Promise<void>")]
  pub fn register_client<'env>(
    &self,
    env: &'env Env,
    client_id: String,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.register_client(client_id).await;
      drop(operation);
      Ok(())
    })
  }

  /// Delivery notification from the serving middleware: the response for
  /// `filename` completed, so record its modules as shipped to that client.
  #[napi(ts_return_type = "Promise<void>")]
  pub fn notify_payload_delivered<'env>(
    &self,
    env: &'env Env,
    filename: String,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.notify_payload_delivered(&filename).await;
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
      return PromiseRaw::resolve(env, ());
    };
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.remove_client(&client_id).await;
      drop(operation);
      Ok(())
    })
  }

  #[napi]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    match self.lifecycle.begin_close() {
      BeginBindingDevEngineClose::Finished(outcome) => {
        PromiseRaw::resolve(env, dev_engine_close_binding_result(outcome, self.cwd.as_ref()))
      }
      BeginBindingDevEngineClose::Acknowledge => PromiseRaw::resolve(env, Either::B(())),
      BeginBindingDevEngineClose::Start { operations_drained, acknowledge: true } => {
        let inner = Arc::clone(&self.inner);
        let close_execution = BindingDevEngineCloseExecutionGuard::new(Arc::clone(&self.lifecycle));
        let background_close = async move {
          let _ =
            execute_binding_dev_engine_close(inner, close_execution, operations_drained).await;
        };
        if let DetachedFutureSpawn::Rejected(background_close) =
          try_spawn_detached_future(background_close)
        {
          drop(background_close);
          return PromiseRaw::reject(
            env,
            napi::Error::from_reason("The async runtime rejected the dev engine close task"),
          );
        }
        PromiseRaw::resolve(env, Either::B(()))
      }
      BeginBindingDevEngineClose::Start { operations_drained, acknowledge: false } => {
        spawn_terminal_binding_dev_engine_close(
          env,
          Arc::clone(&self.inner),
          Arc::clone(&self.lifecycle),
          Arc::clone(&self.cwd),
          operations_drained,
        )
      }
      BeginBindingDevEngineClose::Wait(close_finished) => {
        wait_for_terminal_binding_dev_engine_close(env, Arc::clone(&self.cwd), close_finished)
      }
    }
  }

  #[napi(skip_typescript)]
  pub fn close_terminal<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    match self.lifecycle.begin_terminal_close() {
      BeginBindingDevEngineClose::Finished(outcome) => {
        PromiseRaw::resolve(env, dev_engine_close_binding_result(outcome, self.cwd.as_ref()))
      }
      BeginBindingDevEngineClose::Start { operations_drained, acknowledge: false } => {
        spawn_terminal_binding_dev_engine_close(
          env,
          Arc::clone(&self.inner),
          Arc::clone(&self.lifecycle),
          Arc::clone(&self.cwd),
          operations_drained,
        )
      }
      BeginBindingDevEngineClose::Wait(close_finished) => {
        wait_for_terminal_binding_dev_engine_close(env, Arc::clone(&self.cwd), close_finished)
      }
      BeginBindingDevEngineClose::Acknowledge
      | BeginBindingDevEngineClose::Start { acknowledge: true, .. } => {
        unreachable!("terminal close never acknowledges an active callback")
      }
    }
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
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingLazyChunkOutput>>> {
    let Some(operation) = self.lifecycle.begin_operation() else {
      return PromiseRaw::reject(env, dev_engine_closed_error());
    };
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      // Route the result through `dev_engine_binding_result` (like `run` /
      // `ensure_current_build_finish`) so an `onAdditionalAssets` callback
      // rejection propagates as the original JS error instead of being
      // flattened into a `GenericFailure` string. See dev-callbacks.test.ts
      // "compileEntry awaits onAdditionalAssets and propagates its rejection".
      let result = dev_engine_binding_result(
        inner
          .compile_lazy_entry(module_id, client_id)
          .await
          .map(|output| BindingLazyChunkOutput { code: output.code, filename: output.filename }),
        cwd.as_ref(),
      );
      drop(operation);
      Ok(result)
    })
  }
}

/// The client-facing slice of a lazy-compile result. The carried modules and
/// stamps stay server-side as the engine's pending-payload entry.
#[napi(object)]
pub struct BindingLazyChunkOutput {
  pub code: String,
  pub filename: String,
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
  use crate::types::error::BindingError;
  use rolldown_error::BuildDiagnostic;

  #[test]
  fn lifecycle_close_waits_for_active_operations_and_replays_outcome() {
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());
    let first_operation = lifecycle.begin_operation().expect("engine should start open");
    let second_operation = lifecycle.begin_operation().expect("engine should start open");

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(mut operations_drained),
      acknowledge: false,
    } = lifecycle.begin_close()
    else {
      panic!("close should wait for active operations");
    };
    let BeginBindingDevEngineClose::Wait(mut concurrent_close) = lifecycle.begin_close() else {
      panic!("concurrent close should wait for the terminal result");
    };
    assert!(lifecycle.begin_operation().is_none(), "close must reject new operations");
    assert_eq!(operations_drained.try_recv().expect("sender should remain live"), None);
    assert!(concurrent_close.try_recv().expect("sender should remain live").is_none());

    drop(first_operation);
    assert_eq!(operations_drained.try_recv().expect("sender should remain live"), None);
    drop(second_operation);
    assert_eq!(operations_drained.try_recv().expect("barrier should resolve"), Some(()));
    assert!(concurrent_close.try_recv().expect("sender should remain live").is_none());

    lifecycle.finish_close(Ok(()));
    assert!(matches!(
      concurrent_close.try_recv().expect("close result should resolve"),
      Some(Ok(()))
    ));
    assert!(matches!(lifecycle.begin_close(), BeginBindingDevEngineClose::Finished(Ok(()))));
  }

  #[test]
  fn lifecycle_acknowledges_close_during_owned_callback_and_replays_terminal_outcome() {
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());
    let operation = lifecycle.begin_operation().expect("engine should start open");
    let callback = lifecycle.begin_callback();

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(mut operations_drained),
      acknowledge: true,
    } = lifecycle.begin_close()
    else {
      panic!("callback close should start background cleanup and return an acknowledgement");
    };
    assert!(matches!(lifecycle.begin_close(), BeginBindingDevEngineClose::Acknowledge));
    let BeginBindingDevEngineClose::Wait(mut terminal_close) = lifecycle.begin_terminal_close()
    else {
      panic!("terminal close should wait even while the callback is active");
    };

    drop(callback);
    drop(operation);
    assert_eq!(operations_drained.try_recv().expect("barrier should resolve"), Some(()));

    lifecycle.finish_close(Ok(()));
    assert!(matches!(
      terminal_close.try_recv().expect("terminal close should resolve"),
      Some(Ok(()))
    ));
    assert!(matches!(lifecycle.begin_close(), BeginBindingDevEngineClose::Finished(Ok(()))));
  }

  #[test]
  fn lifecycle_terminal_close_started_during_callback_never_acknowledges() {
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());
    let operation = lifecycle.begin_operation().expect("engine should start open");
    let callback = lifecycle.begin_callback();

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(mut operations_drained),
      acknowledge: false,
    } = lifecycle.begin_terminal_close()
    else {
      panic!("terminal close should start without acknowledging the callback");
    };
    assert!(matches!(lifecycle.begin_close(), BeginBindingDevEngineClose::Acknowledge));

    drop(callback);
    drop(operation);
    assert_eq!(operations_drained.try_recv().expect("barrier should resolve"), Some(()));
    lifecycle.finish_close(Ok(()));
    assert!(matches!(
      lifecycle.begin_terminal_close(),
      BeginBindingDevEngineClose::Finished(Ok(()))
    ));
  }

  #[test]
  fn lifecycle_can_retry_when_close_transport_fails_to_start() {
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());
    let operation = lifecycle.begin_operation().expect("engine should start open");

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(first_operations_drained),
      acknowledge: false,
    } = lifecycle.begin_close()
    else {
      panic!("first close should start");
    };
    drop(first_operations_drained);
    lifecycle.abort_close_start();

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(mut retry_operations_drained),
      acknowledge: false,
    } = lifecycle.begin_close()
    else {
      panic!("transport setup failure must leave close retryable");
    };
    drop(operation);
    assert_eq!(
      retry_operations_drained.try_recv().expect("retry barrier should resolve"),
      Some(())
    );
  }

  #[test]
  fn lifecycle_cancelled_close_executor_wakes_waiters_and_allows_retry() {
    let lifecycle = Arc::new(BindingDevEngineLifecycle::new());
    let operation = lifecycle.begin_operation().expect("engine should start open");
    let callback = lifecycle.begin_callback();

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(first_operations_drained),
      acknowledge: true,
    } = lifecycle.begin_close()
    else {
      panic!("callback close should start an acknowledged executor");
    };
    let close_execution = BindingDevEngineCloseExecutionGuard::new(Arc::clone(&lifecycle));
    let BeginBindingDevEngineClose::Wait(mut terminal_close) = lifecycle.begin_terminal_close()
    else {
      panic!("terminal close should wait for the active executor");
    };

    drop(close_execution);
    assert!(
      terminal_close.try_recv().is_err(),
      "cancelling the executor must wake terminal waiters"
    );

    let BeginBindingDevEngineClose::Start {
      operations_drained: Some(mut retry_operations_drained),
      acknowledge: false,
    } = lifecycle.begin_terminal_close()
    else {
      panic!("executor cancellation must leave close retryable");
    };
    drop(first_operations_drained);
    drop(callback);
    drop(operation);
    assert_eq!(
      retry_operations_drained.try_recv().expect("retry barrier should resolve"),
      Some(())
    );
  }

  #[test]
  fn close_transport_preserves_callback_registration_and_close_diagnostics() {
    let outcome = Err(Arc::new(BatchedBuildDiagnostic::new(vec![
      BuildDiagnostic::napi_error(napi::Error::from_reason("intentional callback failure")),
      BuildDiagnostic::from(anyhow::anyhow!("intentional registration failure")),
      BuildDiagnostic::napi_error(napi::Error::from_reason("intentional closeBundle failure")),
    ])));

    let Either::A(errors) = dev_engine_close_binding_result(outcome, Path::new("/project")) else {
      panic!("close failures must use the structured binding-error transport");
    };

    assert_eq!(errors.errors.len(), 3);
    assert!(matches!(errors.errors[0], BindingError::JsError(_)));
    let BindingError::NativeError(registration_error) = &errors.errors[1] else {
      panic!("watch registration failure must remain a native diagnostic");
    };
    assert!(registration_error.message.contains("intentional registration failure"));
    assert!(matches!(errors.errors[2], BindingError::JsError(_)));
  }
}
