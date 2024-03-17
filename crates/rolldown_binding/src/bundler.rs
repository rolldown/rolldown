use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_fs::OsFileSystem;
use tracing::instrument;

use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  types::binding_outputs::BindingOutputs,
  utils::try_init_custom_trace_subscriber,
  NAPI_ENV,
};

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler<OsFileSystem>>,
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
    Self::new_impl(env, input_options, output_options)
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
  pub fn new_impl(
    env: Env,
    input_opts: BindingInputOptions,
    output_opts: BindingOutputOptions,
  ) -> napi::Result<Self> {
    NAPI_ENV.set(&env, || {
      let (opts, plugins) = input_opts.into();

      Ok(Self {
        inner: Mutex::new(NativeBundler::with_plugins(opts?, output_opts.into(), plugins?)),
      })
    })
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn scan_impl(&self) -> napi::Result<()> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let result = bundler_core.scan().await;

    if let Err(err) = result {
      // TODO: better handing errors
      eprintln!("{err:?}");
      return Err(napi::Error::from_reason("Build failed"));
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
      Err(err) => {
        // TODO: better handing errors
        eprintln!("{err:?}");
        return Err(napi::Error::from_reason("Build failed"));
      }
    };

    Ok(outputs.assets.into())
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
      Err(err) => {
        // TODO: better handing errors
        eprintln!("{err:?}");
        return Err(napi::Error::from_reason("Build failed"));
      }
    };

    Ok(outputs.assets.into())
  }
}
