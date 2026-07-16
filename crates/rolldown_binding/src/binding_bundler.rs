// TODO: add reasons about why creating `BindingBundler` instead of reusing `Bundler` of `rolldown` crate.

use crate::{
  classic_bundler::{
    ClassicBundler, ClassicBundlerCloseError, ClassicBundlerCloseFailure,
    ClassicBundlerOperationGuard,
  },
  types::{
    binding_bundler_options::BindingBundlerOptions,
    binding_outputs::{BindingOutputs, to_binding_error},
    error::{BindingError, BindingErrors, BindingResult, NativeError},
  },
  utils::{
    handle_warnings, normalize_binding_options::normalize_binding_options, spawn_boxed_future,
  },
};
use napi::{
  Env, JsValue, Unknown,
  bindgen_prelude::{Array, FnArgs, Function, JsObjectValue, Object, PromiseRaw},
};
use napi_derive::napi;
use rolldown::{Bundle, BundleHandle, BundlerConfig};
use rolldown_error::{BatchedBuildDiagnostic, BuildDiagnostic};
use std::{
  any::Any,
  panic::{AssertUnwindSafe, catch_unwind},
  path::Path,
  sync::Arc,
};

#[napi]
pub struct BindingBundler {
  inner: ClassicBundler,
  last_bundle_handle: Option<BundleHandle>,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> Self {
    let inner = ClassicBundler::new();
    Self { inner, last_bundle_handle: None }
  }

  #[napi]
  pub fn generate<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let normalized = Self::normalize_binding_options(options)?;
    if let Some(result) = Self::validate_hmr_not_allowed(&normalized, "generate") {
      return spawn_boxed_future(env, async move { Ok(result) });
    }

    let (bundle, bundle_handle, operation) = self.create_bundle(normalized)?;
    let operation_handle = bundle_handle.clone();

    let fut = async move {
      let cwd = bundle.options().cwd.clone();
      let options = Arc::clone(bundle.options());
      let bundle_output = match bundle.generate().await {
        Ok(output) => output,
        Err(errs) => {
          let diagnostics = Arc::new(errs.into_vec());
          operation.close_after_operation(operation_handle, Arc::clone(&diagnostics)).await;
          let errors: Vec<BindingError> = diagnostics
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
            .collect();
          return Ok(napi::Either::A(BindingErrors::new(errors)));
        }
      };
      let _operation = operation;

      if let Err(err) = handle_warnings(bundle_output.warnings, &options).await {
        let error = to_binding_error(&err.into(), cwd.clone());
        return Ok(napi::Either::A(BindingErrors::new(vec![error])));
      }

      Ok(napi::Either::B(bundle_output.assets.into()))
    };
    let promise = spawn_boxed_future(env, fut)?;
    self.install_bundle_handle(bundle_handle);
    Ok(promise)
  }

  #[napi]
  pub fn write<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let normalized = Self::normalize_binding_options(options)?;
    if let Some(result) = Self::validate_hmr_not_allowed(&normalized, "write") {
      return spawn_boxed_future(env, async move { Ok(result) });
    }

    let (bundle, bundle_handle, operation) = self.create_bundle(normalized)?;
    let operation_handle = bundle_handle.clone();

    let fut = async move {
      let cwd = bundle.options().cwd.clone();
      let options = Arc::clone(bundle.options());
      let bundle_output = match bundle.write().await {
        Ok(output) => output,
        Err(errs) => {
          let diagnostics = Arc::new(errs.into_vec());
          operation.close_after_operation(operation_handle, Arc::clone(&diagnostics)).await;
          let errors: Vec<BindingError> = diagnostics
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
            .collect();
          return Ok(napi::Either::A(BindingErrors::new(errors)));
        }
      };
      let _operation = operation;

      if let Err(err) = handle_warnings(bundle_output.warnings, &options).await {
        let error = to_binding_error(&err.into(), cwd.clone());
        return Ok(napi::Either::A(BindingErrors::new(vec![error])));
      }

      Ok(napi::Either::B(bundle_output.assets.into()))
    };
    let promise = spawn_boxed_future(env, fut)?;
    self.install_bundle_handle(bundle_handle);
    Ok(promise)
  }

  #[napi]
  pub fn scan<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let normalized = Self::normalize_binding_options(options)?;
    if let Some(result) = Self::validate_hmr_not_allowed(&normalized, "scan") {
      return spawn_boxed_future(env, async move { Ok(result) });
    }

    let (bundle, bundle_handle, operation) = self.create_bundle(normalized)?;
    let operation_handle = bundle_handle.clone();

    let fut = async move {
      let cwd = bundle.options().cwd.clone();
      match bundle.scan().await {
        Ok(()) => {
          let _operation = operation;
          // scan() returns no useful output, just return empty
          Ok(napi::Either::B(()))
        }
        Err(errs) => {
          let diagnostics = Arc::new(errs.into_vec());
          operation.close_after_operation(operation_handle, Arc::clone(&diagnostics)).await;
          let errors: Vec<BindingError> = diagnostics
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
            .collect();
          Ok(napi::Either::A(BindingErrors::new(errors)))
        }
      }
    };
    let promise = spawn_boxed_future(env, fut)?;
    self.install_bundle_handle(bundle_handle);
    Ok(promise)
  }

  #[napi(skip_typescript)]
  pub fn wait_for_failure_close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let wait = self.inner.wait_for_failure_close();
    spawn_boxed_future(env, async move {
      wait.await;
      Ok(())
    })
  }

  #[napi]
  // See internal-docs/rust-classic-bundler/implementation.md.
  // - `Bundler::close()/inner.close()` requires acquiring `&mut self`
  // - Acquiring `&mut self` in async napi `fn` is unsafe, so we must use a sync `fn` here.
  // - But `Bundler::close()/inner.close()` contains async cleanup operations, so we have await its returned future
  // in another async context instead of directly calling `close().await`.
  // - This also affects how the code is written in `Bundler::close()/inner.close()`, see the implementation there for more details.
  pub fn close<'env>(&mut self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let cleanup_fut = self.inner.close();
    let nested = env.spawn_future_with_callback(
      async move { Ok(cleanup_fut.await.err()) },
      |env, error| match error {
        None => PromiseRaw::resolve(env, ()),
        Some(error) => close_rejection_promise(env, &error),
      },
    )?;
    // `nested` was created in this `env`; only its phantom resolved type changes
    // because JavaScript promise resolution assimilates it.
    // SAFETY: `nested` was created in this `env`; only the phantom resolved type changes.
    Ok(unsafe { PromiseRaw::new(env.raw(), nested.raw()) })
  }

  #[napi(skip_typescript)]
  pub fn close_terminal<'env>(
    &mut self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let cleanup_fut = self.inner.close();
    spawn_boxed_future(env, async move { Ok(close_binding_result(cleanup_fut.await)) })
  }

  #[napi(getter)]
  pub fn closed(&self) -> bool {
    self.inner.closed()
  }

  #[napi]
  pub fn get_watch_files(&self) -> Vec<String> {
    self
      .last_bundle_handle
      .as_ref()
      .map(|handle| handle.watch_files().iter().map(|s| s.to_string()).collect())
      .unwrap_or_default()
  }
}

fn close_rejection_promise<'env>(
  env: &'env Env,
  error: &ClassicBundlerCloseError,
) -> napi::Result<PromiseRaw<'env, ()>> {
  let converted = catch_unwind(AssertUnwindSafe(|| try_close_rejection_value(env, error)));
  let rejection = match converted {
    Ok(Ok(rejection)) => rejection,
    Ok(Err(error)) => napi::JsError::from(error).into_unknown(*env),
    Err(payload) => {
      discard_panic_payload(payload);
      napi::JsError::from(napi::Error::from_reason(error.to_string())).into_unknown(*env)
    }
  };
  PromiseRaw::reject(env, rejection)
}

fn try_close_rejection_value<'env>(
  env: &'env Env,
  error: &ClassicBundlerCloseError,
) -> napi::Result<Unknown<'env>> {
  let mut errors = close_binding_errors(error);
  if let Some(js_error) = take_single_js_error(&mut errors) {
    return Ok(js_error.into_unknown(*env));
  }

  if errors.is_empty() {
    return Ok(napi::JsError::from(napi::Error::from_reason(error.to_string())).into_unknown(*env));
  }

  let mut js_errors = env.create_array(u32::try_from(errors.len()).map_err(|_| {
    napi::Error::from_reason("too many close failures to create a JavaScript AggregateError")
  })?)?;
  for (index, error) in errors.into_iter().enumerate() {
    let index = u32::try_from(index)
      .map_err(|_| napi::Error::from_reason("close failure index exceeds JavaScript array size"))?;
    match error {
      BindingError::JsError(error) => js_errors.set(index, error)?,
      BindingError::NativeError(error) => {
        js_errors.set(index, native_close_error_object(env, error)?)?;
      }
    }
  }

  let global = env.get_global()?;
  let aggregate_error = global
    .get_named_property::<Function<FnArgs<(Array<'_>, String)>, Unknown<'_>>>("AggregateError")?
    .new_instance(FnArgs::from((js_errors, error.to_string())))?;
  Ok(aggregate_error)
}

fn take_single_js_error(errors: &mut Vec<BindingError>) -> Option<napi::JsError> {
  if !matches!(errors.as_slice(), [BindingError::JsError(_)]) {
    return None;
  }
  let BindingError::JsError(js_error) = errors.pop().expect("one JavaScript close error") else {
    unreachable!("the singleton error was checked above");
  };
  Some(js_error)
}

fn native_close_error_object(env: &Env, error: NativeError) -> napi::Result<Object<'_>> {
  let NativeError { kind, message, id, exporter, loc, pos } = error;
  let mut object = env.create_error(napi::Error::from_reason(message.clone()))?;
  object.set_named_property("code", kind.clone())?;
  object.set_named_property("kind", kind)?;
  object.set_named_property("message", message)?;
  object.set_named_property("id", id)?;
  object.set_named_property("exporter", exporter)?;
  object.set_named_property("loc", loc)?;
  object.set_named_property("pos", pos)?;
  Ok(object)
}

fn close_binding_errors(error: &ClassicBundlerCloseError) -> Vec<BindingError> {
  let mut errors = Vec::new();
  for failure in error.failures() {
    append_close_failure_binding_errors(
      &mut errors,
      failure,
      failure.cwd().unwrap_or_else(|| error.cwd()),
    );
  }
  errors
}

fn close_binding_result(result: Result<(), Arc<ClassicBundlerCloseError>>) -> BindingResult<()> {
  match result {
    Ok(()) => napi::Either::B(()),
    Err(error) => napi::Either::A(BindingErrors::new(close_binding_errors(&error))),
  }
}

fn append_close_failure_binding_errors(
  output: &mut Vec<BindingError>,
  failure: &ClassicBundlerCloseFailure,
  cwd: &Path,
) {
  let converted = catch_unwind(AssertUnwindSafe(|| {
    let source = failure.source()?;
    if let Some(diagnostics) =
      source.chain().find_map(|cause| cause.downcast_ref::<BatchedBuildDiagnostic>())
      && !diagnostics.is_empty()
    {
      return Some(
        diagnostics
          .iter()
          .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
          .collect(),
      );
    }
    if let Some(diagnostic) =
      source.chain().find_map(|cause| cause.downcast_ref::<BuildDiagnostic>())
    {
      return Some(vec![to_binding_error(diagnostic, cwd.to_path_buf())]);
    }
    source
      .chain()
      .find_map(|cause| cause.downcast_ref::<napi::Error>())
      .map(|error| vec![BindingError::from_napi_error(error)])
  }));

  match converted {
    Ok(Some(errors)) => {
      output.extend(errors);
      return;
    }
    Ok(None) => {}
    Err(payload) => discard_panic_payload(payload),
  }

  output.push(BindingError::NativeError(NativeError {
    kind: "BUNDLER_CLOSE_ERROR".to_string(),
    message: failure.message().to_string(),
    id: None,
    exporter: None,
    loc: None,
    pos: None,
  }));
}

fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
  }
}

impl BindingBundler {
  fn create_bundle(
    &mut self,
    normalized: BundlerConfig,
  ) -> napi::Result<(Bundle, BundleHandle, ClassicBundlerOperationGuard)> {
    let (bundle, operation) = self
      .inner
      .create_bundle(normalized.options, normalized.plugins)
      .map_err(Self::bundle_creation_error)?;
    let handle = bundle.context();
    Ok((bundle, handle, operation))
  }

  fn install_bundle_handle(&mut self, handle: BundleHandle) {
    self.inner.install_bundle_handle(handle.clone());
    self.last_bundle_handle = Some(handle);
  }

  fn bundle_creation_error(error: BatchedBuildDiagnostic) -> napi::Error {
    napi::Error::new(
      napi::Status::GenericFailure,
      error.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
    )
  }

  fn normalize_binding_options(option: BindingBundlerOptions) -> napi::Result<BundlerConfig> {
    #[cfg(not(target_family = "wasm"))]
    let BindingBundlerOptions { input_options, output_options, parallel_plugins_registry } = option;
    #[cfg(target_family = "wasm")]
    let BindingBundlerOptions { input_options, output_options, parallel_plugins_registry: _ } =
      option;

    #[cfg(not(target_family = "wasm"))]
    let worker_count =
      parallel_plugins_registry.as_ref().map(|registry| registry.worker_count).unwrap_or_default();
    #[cfg(not(target_family = "wasm"))]
    let parallel_plugins_map =
      parallel_plugins_registry.map(|registry| registry.take_plugin_values()).transpose()?;

    #[cfg(not(target_family = "wasm"))]
    let worker_manager = if worker_count > 0 {
      use crate::worker_manager::WorkerManager;
      Some(WorkerManager::new(worker_count))
    } else {
      None
    };

    let ret = normalize_binding_options(
      input_options,
      output_options,
      #[cfg(not(target_family = "wasm"))]
      parallel_plugins_map,
      #[cfg(not(target_family = "wasm"))]
      worker_manager,
    )?;

    Ok(ret)
  }

  /// Validates that dev mode is not enabled for the given API.
  /// Returns an error result if dev mode is enabled.
  fn validate_hmr_not_allowed<T>(
    normalized: &BundlerConfig,
    api_name: &str,
  ) -> Option<BindingResult<T>> {
    if normalized.options.experimental.as_ref().and_then(|e| e.dev_mode.as_ref()).is_some() {
      let message = format!(
        "The \"experimental.devMode\" option is only supported with the \"dev\" API. It cannot be used with \"{api_name}\". Please use the \"dev\" API for dev mode functionality."
      );
      let error = rolldown_error::BuildDiagnostic::bundler_initialize_error(message, None);
      let cwd = normalized.options.cwd.clone().unwrap_or_default();
      let binding_error = to_binding_error(&error, cwd);
      Some(napi::Either::A(BindingErrors::new(vec![binding_error])))
    } else {
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{error::Error, fmt, path::PathBuf};

  use super::*;

  #[derive(Debug)]
  struct PanickingSourceError;

  impl fmt::Display for PanickingSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.write_str("error with a panicking source")
    }
  }

  impl Error for PanickingSourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
      panic!("injected source panic");
    }
  }

  #[test]
  fn close_failures_keep_javascript_and_native_diagnostics_separate() {
    let js_failure = ClassicBundlerCloseFailure::from_error(
      "closeBundle failed",
      anyhow::Error::new(napi::Error::from_reason("javascript close failure")),
    );
    let native_failure = ClassicBundlerCloseFailure::from_message("devtools session flush failed");

    let mut errors = close_binding_errors(&ClassicBundlerCloseError::new(
      PathBuf::new(),
      vec![js_failure, native_failure],
    ));

    assert!(matches!(errors.remove(0), BindingError::JsError(_)));
    match errors.remove(0) {
      BindingError::NativeError(error) => {
        assert_eq!(error.kind, "BUNDLER_CLOSE_ERROR");
        assert_eq!(error.message, "devtools session flush failed");
      }
      BindingError::JsError(_) => panic!("native failure must remain a separate native diagnostic"),
    }
  }

  #[test]
  fn singleton_native_close_failure_is_not_consumed_while_inspecting_for_a_js_error() {
    let error = ClassicBundlerCloseError::new(
      PathBuf::new(),
      vec![ClassicBundlerCloseFailure::from_message("native close failure")],
    );
    let mut errors = close_binding_errors(&error);

    assert!(take_single_js_error(&mut errors).is_none());
    let [BindingError::NativeError(error)] = errors.as_slice() else {
      panic!("the singleton native failure must remain available for conversion");
    };
    assert_eq!(error.message, "native close failure");
  }

  #[test]
  fn close_failure_expands_a_batched_build_diagnostic() {
    let failure = ClassicBundlerCloseFailure::from_error(
      "closeBundle failed",
      anyhow::Error::new(BatchedBuildDiagnostic::new(vec![
        BuildDiagnostic::bundler_initialize_error("first diagnostic".to_string(), None),
        BuildDiagnostic::bundler_initialize_error("second diagnostic".to_string(), None),
      ])),
    );
    let mut errors = Vec::new();

    append_close_failure_binding_errors(&mut errors, &failure, Path::new(""));

    assert_eq!(errors.len(), 2);
    assert!(errors.into_iter().all(|error| matches!(error, BindingError::NativeError(_))));
  }

  #[test]
  fn retained_close_failure_uses_its_originating_output_cwd() {
    let root = std::env::temp_dir().join("rolldown-close-cwd");
    let originating_cwd = root.join("first");
    let latest_cwd = root.join("latest");
    let missing_entry = originating_cwd.join("src/missing.js");
    let failure = ClassicBundlerCloseFailure::from_error(
      "closeBundle failed",
      anyhow::Error::new(BuildDiagnostic::unresolved_entry(&missing_entry, None)),
    )
    .with_cwd(originating_cwd);
    let error = ClassicBundlerCloseError::new(latest_cwd, vec![failure]);

    let binding_errors = close_binding_errors(&error);
    let [BindingError::NativeError(error)] = binding_errors.as_slice() else {
      panic!("the retained build diagnostic must remain a native binding error");
    };
    assert!(
      error.message.contains("src/missing.js"),
      "the diagnostic should be rendered relative to its originating cwd: {}",
      error.message
    );
    assert!(
      !error.message.contains("../first/"),
      "the latest output cwd must not be used for an older failure: {}",
      error.message
    );
  }

  #[test]
  fn close_failure_source_panic_falls_back_on_every_conversion() {
    let failure = ClassicBundlerCloseFailure::from_error(
      "closeBundle failed",
      anyhow::Error::new(PanickingSourceError),
    );

    for _ in 0..2 {
      let mut errors = Vec::new();
      append_close_failure_binding_errors(&mut errors, &failure, Path::new(""));

      let [BindingError::NativeError(error)] = errors.as_slice() else {
        panic!("panicking source must produce one native fallback");
      };
      assert_eq!(error.kind, "BUNDLER_CLOSE_ERROR");
      assert_eq!(
        error.message,
        "closeBundle failed: error formatting panicked: injected source panic"
      );
    }
  }
}
