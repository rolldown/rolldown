use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_common::BatchedErrors;
use rolldown_error::BuildError;
use tracing::instrument;

use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  types::binding_outputs::BindingOutputs,
  utils::{normalize_binding_options::normalize_binding_options, try_init_custom_trace_subscriber},
};

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler>,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  pub fn new(
    env: Env,
    input_options: BindingInputOptions,
    output_options: BindingOutputOptions,
  ) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);
    let ret = normalize_binding_options(input_options, output_options)?;

    Ok(Self {
      inner: Mutex::new(NativeBundler::with_plugins(
        ret.input_options,
        ret.output_options,
        ret.plugins,
      )),
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

    let result = bundler_core.scan().await;

    if let Err(errs) = result {
      return Err(Self::handle_errors(errs));
    }

    Ok(())
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn write_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let maybe_outputs = bundler_core.write().await;

    let outputs = match maybe_outputs {
      Ok(outputs) => outputs,
      Err(errs) => return Err(Self::handle_errors(errs)),
    };

    Self::handle_warnings(outputs.warnings);

    Ok(BindingOutputs::new(outputs.assets))
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self) -> napi::Result<BindingOutputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let maybe_outputs = bundler_core.generate().await;

    let outputs = match maybe_outputs {
      Ok(outputs) => outputs,
      Err(errs) => return Err(Self::handle_errors(errs)),
    };

    Self::handle_warnings(outputs.warnings);

    Ok(BindingOutputs::new(outputs.assets))
  }

  fn handle_errors(errs: BatchedErrors) -> napi::Error {
    errs.into_iter().for_each(|err| {
      eprintln!("{}", err.into_diagnostic().to_color_string());
    });
    napi::Error::from_reason("Build failed")
  }

  #[allow(clippy::print_stdout)]
  fn handle_warnings(warnings: Vec<BuildError>) {
    warnings.into_iter().for_each(|err| {
      println!("{}", err.into_diagnostic().to_color_string());
    });
  }
}
