use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use crate::worker_manager::WorkerManager;
use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
  types::{binding_hmr_output::BindingHmrOutput, binding_outputs::BindingOutputs},
  utils::{
    handle_result, normalize_binding_options::normalize_binding_options,
    try_init_custom_trace_subscriber,
  },
};
use napi::{Env, tokio::sync::Mutex};
use napi_derive::napi;
use rolldown::{Bundler as NativeBundler, LogLevel, NormalizedBundlerOptions};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosticOptions, filter_out_disabled_diagnostics,
};

#[napi(object, object_to_js = false)]
pub struct BindingBundlerOptions<'env> {
  pub input_options: BindingInputOptions<'env>,
  pub output_options: BindingOutputOptions<'env>,
  pub parallel_plugins_registry: Option<ParallelJsPluginRegistry>,
}

#[napi]
pub struct BindingBundlerImpl {
  inner: Arc<Mutex<NativeBundler>>,
}

#[napi]
impl BindingBundlerImpl {
  #[napi(constructor)]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn new(env: Env, option: BindingBundlerOptions) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);

    let BindingBundlerOptions { input_options, output_options, parallel_plugins_registry } = option;

    #[cfg(not(target_family = "wasm"))]
    let worker_count =
      parallel_plugins_registry.as_ref().map(|registry| registry.worker_count).unwrap_or_default();
    #[cfg(not(target_family = "wasm"))]
    let parallel_plugins_map =
      parallel_plugins_registry.map(|registry| registry.take_plugin_values());

    #[cfg(not(target_family = "wasm"))]
    let worker_manager =
      if worker_count > 0 { Some(WorkerManager::new(worker_count)) } else { None };

    let ret = normalize_binding_options(
      input_options,
      output_options,
      #[cfg(not(target_family = "wasm"))]
      parallel_plugins_map,
      #[cfg(not(target_family = "wasm"))]
      worker_manager,
    )?;

    Ok(Self {
      inner: Arc::new(Mutex::new(NativeBundler::with_plugins(ret.bundler_options, ret.plugins))),
    })
  }

  pub fn new_with_bundler(inner: Arc<Mutex<NativeBundler>>) -> Self {
    Self { inner }
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn write(&self) -> napi::Result<BindingOutputs> {
    self.write_impl().await
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&self) -> napi::Result<BindingOutputs> {
    self.generate_impl().await
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(&self) -> napi::Result<BindingOutputs> {
    self.scan_impl().await
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&self) -> napi::Result<()> {
    self.close_impl().await
  }

  #[napi(getter)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn get_closed(&self) -> napi::Result<bool> {
    napi::bindgen_prelude::block_on(async { self.get_closed_impl().await })
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn get_watch_files(&self) -> napi::Result<Vec<String>> {
    let bundler_core = self.inner.lock().await;
    Ok(bundler_core.get_watch_files().iter().map(|s| s.to_string()).collect())
  }

  #[napi]
  pub async fn generate_hmr_patch(
    &self,
    changed_files: Vec<String>,
  ) -> napi::Result<BindingHmrOutput> {
    let mut bundler_core = self.inner.lock().await;
    let result = bundler_core.generate_hmr_patch(changed_files).await;
    match result {
      Ok(output) => Ok(output.into()),
      Err(errs) => {
        Ok(BindingHmrOutput::from_errors(errs.into_vec(), bundler_core.options().cwd.clone()))
      }
    }
  }

  #[napi]
  pub async fn hmr_invalidate(
    &self,
    file: String,
    first_invalidated_by: Option<String>,
  ) -> napi::Result<BindingHmrOutput> {
    let mut bundler_core = self.inner.lock().await;
    let result = bundler_core.hmr_invalidate(file, first_invalidated_by).await;
    match result {
      Ok(output) => Ok(output.into()),
      Err(errs) => {
        Ok(BindingHmrOutput::from_errors(errs.into_vec(), bundler_core.options().cwd.clone()))
      }
    }
  }
}

impl BindingBundlerImpl {
  #[allow(clippy::significant_drop_tightening)]
  pub async fn scan_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.lock().await;
    let output = Self::handle_result(bundler_core.scan(vec![]).await, bundler_core.options());

    match output {
      Ok(output) => {
        self.handle_warnings(output.warnings, bundler_core.options()).await;
      }
      Err(outputs) => {
        return Ok(outputs);
      }
    }

    Ok(vec![].into())
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn write_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.lock().await;

    let outputs = match bundler_core.write().await {
      Ok(outputs) => outputs,
      Err(errs) => return Ok(Self::handle_errors(errs.into_vec(), bundler_core.options())),
    };

    self.handle_warnings(outputs.warnings, bundler_core.options()).await;

    Ok(outputs.assets.into())
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.lock().await;

    let bundle_output = match bundler_core.generate().await {
      Ok(output) => output,
      Err(errs) => return Ok(Self::handle_errors(errs.into_vec(), bundler_core.options())),
    };

    self.handle_warnings(bundle_output.warnings, bundler_core.options()).await;

    Ok(bundle_output.assets.into())
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn close_impl(&self) -> napi::Result<()> {
    let mut bundler_core = self.inner.lock().await;

    handle_result(bundler_core.close().await)?;

    Ok(())
  }

  pub fn into_inner(self) -> Arc<Mutex<NativeBundler>> {
    self.inner
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn get_closed_impl(&self) -> napi::Result<bool> {
    let bundler_core = self.inner.lock().await;

    Ok(bundler_core.closed)
  }

  fn handle_errors(
    errs: Vec<BuildDiagnostic>,
    options: &NormalizedBundlerOptions,
  ) -> BindingOutputs {
    BindingOutputs::from_errors(errs, options.cwd.clone())
  }

  fn handle_result<T>(
    result: BuildResult<T>,
    options: &NormalizedBundlerOptions,
  ) -> Result<T, BindingOutputs> {
    result.map_err(|e| Self::handle_errors(e.into_vec(), options))
  }

  #[allow(clippy::print_stdout, unused_must_use)]
  async fn handle_warnings(
    &self,
    warnings: Vec<BuildDiagnostic>,
    options: &NormalizedBundlerOptions,
  ) {
    if options.log_level == Some(LogLevel::Silent) {
      return;
    }
    if let Some(on_log) = options.on_log.as_ref() {
      for warning in filter_out_disabled_diagnostics(warnings, &options.checks) {
        on_log
          .call(
            LogLevel::Warn,
            rolldown::Log {
              id: warning.id(),
              exporter: warning.exporter(),
              code: warning.kind().to_string(),
              message: warning
                .to_diagnostic_with(&DiagnosticOptions { cwd: options.cwd.clone() })
                .to_color_string(),
            },
          )
          .await;
      }
    }
  }
}
