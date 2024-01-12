use dashmap::DashMap;
use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_fs::OsFileSystem;
use tracing::instrument;

use crate::{
  options::InputOptions,
  options::{JsAdapterPlugin, OutputOptions, PluginOptions},
  output::Outputs,
  utils::try_init_custom_trace_subscriber,
  NAPI_ENV,
};

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler<OsFileSystem>>,
  output_plugins_map: DashMap<u32, Vec<rolldown::BoxPlugin>>,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  pub fn new(env: Env, input_opts: InputOptions) -> napi::Result<Self> {
    try_init_custom_trace_subscriber(env);
    Self::new_impl(env, input_opts)
  }

  // The `write/generate` is async function, it isn't allow has `PluginOptions` argument, because the `JsFunction` isn't `Send`.
  // We can using `ThreadsafeFunction` to replace `JsFunction` for `PluginOptions`, see this issue https://github.com/napi-rs/napi-rs/issues/1644,
  // but the `ThreadsafeFunction` isn't `ToNapiValue`, we only can be using it directly at the async function, it is ugly for many plugin hook.
  // So here using a map to store `plugin` using sync function and take it to `write/generate` call.
  #[napi]
  pub fn set_output_plugins(
    &mut self,
    index: u32,
    plugins: Vec<PluginOptions>,
  ) -> napi::Result<()> {
    self.output_plugins_map.insert(
      index,
      plugins.into_iter().map(JsAdapterPlugin::new_boxed).collect::<napi::Result<Vec<_>>>()?,
    );
    Ok(())
  }

  #[napi]
  pub async fn write(&self, index: u32, opts: OutputOptions) -> napi::Result<Outputs> {
    let (_, plugins) = self.output_plugins_map.remove(&index).expect("should have output");
    self.write_impl(opts, plugins).await
  }

  #[napi]
  pub async fn generate(&self, index: u32, opts: OutputOptions) -> napi::Result<Outputs> {
    let (_, plugins) = self.output_plugins_map.remove(&index).expect("should have output");
    self.generate_impl(opts, plugins).await
  }

  #[napi]
  pub async fn build(&self) -> napi::Result<()> {
    self.build_impl().await
  }

  #[napi]
  pub async fn scan(&self) -> napi::Result<()> {
    self.scan_impl().await
  }
}

impl Bundler {
  pub fn new_impl(env: Env, input_opts: InputOptions) -> napi::Result<Self> {
    NAPI_ENV.set(&env, || {
      let (opts, plugins) = input_opts.into();

      Ok(Self {
        inner: Mutex::new(NativeBundler::with_plugins(opts?, plugins?)),
        output_plugins_map: DashMap::default(),
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
  pub async fn write_impl(
    &self,
    output_opts: OutputOptions,
    plugins: Vec<rolldown::BoxPlugin>,
  ) -> napi::Result<Outputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let maybe_outputs = bundler_core.write(output_opts.into(), plugins).await;

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
  pub async fn generate_impl(
    &self,
    output_opts: OutputOptions,
    plugins: Vec<rolldown::BoxPlugin>,
  ) -> napi::Result<Outputs> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let maybe_outputs = bundler_core.generate(output_opts.into(), plugins).await;

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
