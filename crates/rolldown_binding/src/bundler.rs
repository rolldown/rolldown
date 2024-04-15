use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
  types::binding_outputs::BindingOutputs,
  utils::{normalize_binding_options::normalize_binding_options, try_init_custom_trace_subscriber},
  worker_manager::WorkerManager,
};
use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_error::BuildError;
use tracing::instrument;

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler>,
  log_level: String,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  pub fn new(
    env: Env,
    mut input_options: BindingInputOptions,
    output_options: BindingOutputOptions,
    parallel_plugins_registry: Option<ParallelJsPluginRegistry>,
  ) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);

    let log_level = input_options.log_level.take().unwrap_or_else(|| "info".to_string());

    let worker_count =
      parallel_plugins_registry.as_ref().map(|registry| registry.worker_count).unwrap_or_default();
    let parallel_plugins_map =
      parallel_plugins_registry.map(|registry| registry.take_plugin_values());

    let worker_manager =
      if worker_count > 0 { Some(WorkerManager::new(worker_count)) } else { None };

    let ret = normalize_binding_options(
      input_options,
      output_options,
      parallel_plugins_map,
      worker_manager,
    )?;

    Ok(Self {
      inner: Mutex::new(NativeBundler::with_plugins(ret.bundler_options, ret.plugins)),
      log_level,
    })
  }

  #[napi]
  pub async fn write(&self) -> napi::Result<BindingOutputs> {
    self.write_impl().await
  }

  #[napi]
  pub async fn generate(&self) -> napi::Result<BindingOutputs> {
    self.generate_impl().await
  }

  #[napi]
  pub async fn scan(&self) -> napi::Result<()> {
    self.scan_impl().await
  }
}

impl Bundler {
  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn scan_impl(&self) -> napi::Result<()> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let output = Self::handle_result(bundler_core.scan().await)?;

    if !output.errors.is_empty() {
      return Err(Self::handle_errors(output.errors));
    }

    self.handle_warnings(output.warnings);

    Ok(())
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn write_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let outputs = Self::handle_result(bundler_core.write().await)?;

    if !outputs.errors.is_empty() {
      return Err(Self::handle_errors(outputs.errors));
    }

    self.handle_warnings(outputs.warnings);

    Ok(BindingOutputs::new(outputs.assets))
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let outputs = Self::handle_result(bundler_core.generate().await)?;

    if !outputs.errors.is_empty() {
      return Err(Self::handle_errors(outputs.errors));
    }

    self.handle_warnings(outputs.warnings);

    Ok(BindingOutputs::new(outputs.assets))
  }

  fn handle_result<T>(result: rolldown_error::Result<T>) -> napi::Result<T> {
    result.map_err(|e| napi::Error::from_reason(format!("Rolldown internal error: {e}")))
  }

  fn handle_errors(errs: Vec<BuildError>) -> napi::Error {
    errs.into_iter().for_each(|err| {
      eprintln!("{}", err.into_diagnostic().to_color_string());
    });
    napi::Error::from_reason("Build failed")
  }

  #[allow(clippy::print_stdout)]
  fn handle_warnings(&self, warnings: Vec<BuildError>) {
    match self.log_level.as_str() {
      "silent" => return,
      _ => {}
    }
    warnings.into_iter().for_each(|err| {
      println!("{}", err.into_diagnostic().to_color_string());
    });
  }
}
