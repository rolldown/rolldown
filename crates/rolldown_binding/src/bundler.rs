use std::{path::PathBuf, sync::Arc};

#[cfg(not(target_family = "wasm"))]
use crate::worker_manager::WorkerManager;
use crate::{
  options::{BindingInputOptions, BindingOnLog, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
  types::{
    binding_log::BindingLog, binding_log_level::BindingLogLevel, binding_outputs::BindingOutputs,
  },
  utils::{
    handle_result, normalize_binding_options::normalize_binding_options,
    try_init_custom_trace_subscriber,
  },
};
use napi::{Env, tokio::sync::Mutex};
use napi_derive::napi;
use rolldown::{Bundler as NativeBundler, NormalizedBundlerOptions};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosticOptions, filter_out_disabled_diagnostics,
};

#[napi(object, object_to_js = false)]
pub struct BindingBundlerOptions {
  pub input_options: BindingInputOptions,
  pub output_options: BindingOutputOptions,
  pub parallel_plugins_registry: Option<ParallelJsPluginRegistry>,
}

#[napi]
pub struct Bundler {
  inner: Arc<Mutex<NativeBundler>>,
  on_log: BindingOnLog,
  log_level: BindingLogLevel,
  cwd: PathBuf,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn new(env: Env, option: BindingBundlerOptions) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);

    let BindingBundlerOptions { mut input_options, output_options, parallel_plugins_registry } =
      option;

    let log_level = input_options.log_level;
    let on_log = input_options.on_log.take();

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
      cwd: ret.bundler_options.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap()),
      inner: Arc::new(Mutex::new(NativeBundler::with_plugins(ret.bundler_options, ret.plugins))),
      log_level,
      on_log,
    })
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

  #[napi(getter)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn get_watch_files(&self) -> napi::Result<Vec<String>> {
    napi::bindgen_prelude::block_on(async {
      let bundler_core = self.inner.lock().await;
      Ok(bundler_core.get_watch_files().iter().map(|s| s.to_string()).collect())
    })
  }

  #[napi]
  pub async fn generate_hmr_patch(&self, changed_files: Vec<String>) -> String {
    let mut bundler_core = self.inner.lock().await;
    bundler_core.generate_hmr_patch(changed_files).await.expect("Failed to generate HMR patch")
  }
}

impl Bundler {
  #[allow(clippy::significant_drop_tightening)]
  pub async fn scan_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.lock().await;
    let output = self.handle_result(bundler_core.scan(vec![]).await);

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
      Err(errs) => return Ok(self.handle_errors(errs.into_vec())),
    };

    self.handle_warnings(outputs.warnings, bundler_core.options()).await;

    Ok(outputs.assets.into())
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.lock().await;

    let bundle_output = match bundler_core.generate().await {
      Ok(output) => output,
      Err(errs) => return Ok(self.handle_errors(errs.into_vec())),
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

  fn handle_errors(&self, errs: Vec<BuildDiagnostic>) -> BindingOutputs {
    BindingOutputs::from_errors(errs, self.cwd.clone())
  }

  fn handle_result<T>(&self, result: BuildResult<T>) -> Result<T, BindingOutputs> {
    result.map_err(|e| self.handle_errors(e.into_vec()))
  }

  #[allow(clippy::print_stdout, unused_must_use)]
  async fn handle_warnings(
    &self,
    mut warnings: Vec<BuildDiagnostic>,
    options: &NormalizedBundlerOptions,
  ) {
    if self.log_level == BindingLogLevel::Silent {
      return;
    }
    warnings = filter_out_disabled_diagnostics(warnings, &options.checks);
    if let Some(on_log) = self.on_log.as_ref() {
      for warning in warnings {
        on_log
          .call_async(
            (
              BindingLogLevel::Warn.to_string(),
              BindingLog {
                code: warning.kind().to_string(),
                message: warning
                  .to_diagnostic_with(&DiagnosticOptions { cwd: self.cwd.clone() })
                  .to_color_string(),
                id: warning.id(),
                exporter: warning.exporter(),
              },
            )
              .into(),
          )
          .await;
      }
    }
  }
}
