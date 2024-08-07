use std::path::PathBuf;

#[cfg(not(target_family = "wasm"))]
use crate::worker_manager::WorkerManager;
use crate::{
  options::{BindingInputOptions, BindingOnLog, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
  types::{
    binding_log::BindingLog, binding_log_level::BindingLogLevel,
    binding_outputs::FinalBindingOutputs,
  },
  utils::{normalize_binding_options::normalize_binding_options, try_init_custom_trace_subscriber},
};
use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler>,
  on_log: BindingOnLog,
  log_level: Option<BindingLogLevel>,
  cwd: PathBuf,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn new(
    env: Env,
    mut input_options: BindingInputOptions,
    output_options: BindingOutputOptions,
    parallel_plugins_registry: Option<ParallelJsPluginRegistry>,
  ) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);

    let log_level = input_options.log_level.take();
    let on_log = input_options.on_log.take();

    #[cfg(target_family = "wasm")]
    // if we don't perform this warmup, the following call to `std::fs` will stuck
    if let Ok(_) = std::fs::metadata(std::env::current_dir()?) {};

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
      env,
      input_options,
      output_options,
      #[cfg(not(target_family = "wasm"))]
      parallel_plugins_map,
      #[cfg(not(target_family = "wasm"))]
      worker_manager,
    )?;

    Ok(Self {
      cwd: ret.bundler_options.cwd.clone().unwrap_or_else(|| std::env::current_dir().unwrap()),
      inner: Mutex::new(NativeBundler::with_plugins(ret.bundler_options, ret.plugins)),
      log_level,
      on_log,
    })
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn write(&self) -> napi::Result<FinalBindingOutputs> {
    self.write_impl().await
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&self) -> napi::Result<FinalBindingOutputs> {
    self.generate_impl().await
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(&self) -> napi::Result<()> {
    self.scan_impl().await
  }
}

impl Bundler {
  #[allow(clippy::significant_drop_tightening)]
  pub async fn scan_impl(&self) -> napi::Result<()> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let output = Self::handle_result(bundler_core.scan().await)?;

    match output {
      Ok(output) => {
        self.handle_warnings(output.warnings).await;
      }
      Err(errs) => {
        return Err(self.handle_errors(errs));
      }
    }

    Ok(())
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn write_impl(&self) -> napi::Result<FinalBindingOutputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let outputs = Self::handle_result(bundler_core.write().await)?;

    if !outputs.errors.is_empty() {
      return Err(self.handle_errors(outputs.errors));
    }

    self.handle_warnings(outputs.warnings).await;

    Ok(FinalBindingOutputs::new(outputs.assets))
  }

  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self) -> napi::Result<FinalBindingOutputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let outputs = Self::handle_result(bundler_core.generate().await)?;

    if !outputs.errors.is_empty() {
      return Err(self.handle_errors(outputs.errors));
    }

    self.handle_warnings(outputs.warnings).await;

    Ok(FinalBindingOutputs::new(outputs.assets))
  }

  fn handle_result<T>(result: anyhow::Result<T>) -> napi::Result<T> {
    result.map_err(|e| napi::Error::from_reason(format!("Rolldown internal error: {e}")))
  }

  fn handle_errors(&self, errs: Vec<BuildDiagnostic>) -> napi::Error {
    errs.into_iter().for_each(|err| {
      eprintln!(
        "{}",
        err.into_diagnostic_with(&DiagnosticOptions { cwd: self.cwd.clone() }).to_color_string()
      );
    });
    napi::Error::from_reason("Build failed")
  }

  #[allow(clippy::print_stdout, unused_must_use)]
  async fn handle_warnings(&self, warnings: Vec<BuildDiagnostic>) {
    if let Some(log_level) = self.log_level {
      if log_level == BindingLogLevel::Silent {
        return;
      }
    }

    if let Some(on_log) = self.on_log.as_ref() {
      for warning in warnings {
        on_log
          .call_async((
            BindingLogLevel::Warn.to_string(),
            BindingLog {
              code: warning.kind().to_string(),
              message: warning
                .into_diagnostic_with(&DiagnosticOptions { cwd: self.cwd.clone() })
                .to_color_string(),
            },
          ))
          .await;
      }
    }
  }
}
