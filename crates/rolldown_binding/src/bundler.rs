use std::{
  ops::{Deref, DerefMut},
  sync::Arc,
};

use napi::{
  bindgen_prelude::{External, FromNapiRef, FromNapiValue},
  tokio::sync::Mutex,
  Env, JsUnknown, NapiValue, ValueType,
};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_common::BatchedErrors;
use rolldown_error::BuildError;
use tracing::instrument;

use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
  types::binding_outputs::BindingOutputs,
  utils::{normalize_binding_options::normalize_binding_options, try_init_custom_trace_subscriber},
  worker_manager::WorkerManager,
};

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
    parallel_plugins_registry: OptionExtended<External<Arc<ParallelJsPluginRegistry>>>,
  ) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);

    let log_level = input_options.log_level.take().unwrap_or_else(|| "info".to_string());

    let worker_count =
      parallel_plugins_registry.map(|registry| registry.worker_count).unwrap_or_default();
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

    self.handle_warnings(outputs.warnings);

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

    self.handle_warnings(outputs.warnings);

    Ok(BindingOutputs::new(outputs.assets))
  }

  fn handle_errors(errs: BatchedErrors) -> napi::Error {
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

pub struct OptionExtended<'a, T>(Option<&'a T>);

impl<'a, T> Deref for OptionExtended<'a, T> {
  type Target = Option<&'a T>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<'a, T> DerefMut for OptionExtended<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<T: FromNapiRef> FromNapiValue for OptionExtended<'static, T> {
  unsafe fn from_napi_value(
    env: napi::sys::napi_env,
    napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    let unknown = JsUnknown::from_raw_unchecked(env, napi_val);
    let val = match unknown.get_type()? {
      ValueType::Undefined => None,
      _ => Some(T::from_napi_ref(env, napi_val)?),
    };
    Ok(Self(val))
  }
}
