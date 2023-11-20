use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_fs::OsFileSystem;
use tracing::instrument;

use crate::{
  options::InputOptions, options::OutputOptions, output::Outputs,
  utils::try_init_custom_trace_subscriber, NAPI_ENV,
};

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler<OsFileSystem>>,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  pub fn new(env: Env, input_opts: InputOptions) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);
    Self::new_impl(env, input_opts)
  }

  #[napi]
  pub async fn write(&self, opts: OutputOptions) -> napi::Result<Outputs> {
    self.write_impl(opts).await
  }

  #[napi]
  pub async fn generate(&self, opts: OutputOptions) -> napi::Result<Outputs> {
    self.generate_impl(opts).await
  }

  #[napi]
  pub async fn build(&self) -> napi::Result<()> {
    self.build_impl().await
  }
}

impl Bundler {
  pub fn new_impl(env: Env, input_opts: InputOptions) -> napi::Result<Self> {
    NAPI_ENV.set(&env, || {
      let (opts, plugins) = input_opts.into();

      Ok(Self { inner: Mutex::new(NativeBundler::with_plugins(opts?, plugins?)) })
    })
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn build_impl(&self) -> napi::Result<()> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let result = bundler_core.build().await;

    if let Err(err) = result {
      // TODO: better handing errors
      for err in err {
        eprintln!("{err:?}");
      }
      return Err(napi::Error::from_reason("Build failed"));
    }

    Ok(())
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn write_impl(&self, output_opts: OutputOptions) -> napi::Result<Outputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let maybe_outputs = bundler_core.write(output_opts.into()).await;

    let outputs = match maybe_outputs {
      Ok(outputs) => outputs,
      Err(err) => {
        // TODO: better handing errors
        for err in err {
          eprintln!("{err:?}");
        }
        return Err(napi::Error::from_reason("Build failed"));
      }
    };

    Ok(outputs.assets.into())
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self, output_opts: OutputOptions) -> napi::Result<Outputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let maybe_outputs = bundler_core.generate(output_opts.into()).await;

    let outputs = match maybe_outputs {
      Ok(outputs) => outputs,
      Err(err) => {
        // TODO: better handing errors
        for err in err {
          eprintln!("{err:?}");
        }
        return Err(napi::Error::from_reason("Build failed"));
      }
    };

    Ok(outputs.assets.into())
  }
}
