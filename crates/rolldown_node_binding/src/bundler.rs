use std::sync::Arc;

use napi::{tokio::sync::Mutex, Env};
use napi_derive::napi;
use rolldown::Bundler as NativeBundler;
use rolldown_fs::{FileSystemExt, FileSystemOs};
use tracing::instrument;

use crate::{
  options::InputOptions,
  options::{resolve_input_options, resolve_output_options, OutputOptions},
  output_chunk::OutputChunk,
  utils::init_custom_trace_subscriber,
  NAPI_ENV,
};

#[napi]
pub struct Bundler {
  inner: Mutex<NativeBundler>,
}

#[napi]
impl Bundler {
  #[napi(constructor)]
  pub fn new(env: Env, input_opts: InputOptions) -> napi::Result<Self> {
    init_custom_trace_subscriber(env);
    Self::new_impl(env, input_opts)
  }

  #[napi]
  pub async fn write(&self, opts: OutputOptions) -> napi::Result<Vec<OutputChunk>> {
    self.write_impl(opts).await
  }

  #[napi]
  pub async fn generate(&self, opts: OutputOptions) -> napi::Result<Vec<OutputChunk>> {
    self.generate_impl(opts).await
  }
}

impl Bundler {
  pub fn new_impl(env: Env, input_opts: InputOptions) -> napi::Result<Self> {
    NAPI_ENV.set(&env, || {
      let (input_opts, plugins) = resolve_input_options(input_opts)?;
      Ok(Self {
        inner: Mutex::new(NativeBundler::with_plugins(input_opts, plugins, Arc::new(FileSystemOs))),
      })
    })
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn write_impl(&self, opts: OutputOptions) -> napi::Result<Vec<OutputChunk>> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let binding_opts = resolve_output_options(opts)?;

    let maybe_outputs = bundler_core.write(binding_opts).await;

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

    let output_chunks = outputs
      .into_iter()
      .map(|asset| OutputChunk { code: asset.content, file_name: asset.file_name })
      .collect::<Vec<_>>();
    Ok(output_chunks)
  }

  #[instrument(skip_all)]
  #[allow(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self, opts: OutputOptions) -> napi::Result<Vec<OutputChunk>> {
    let mut bundler_core = self.inner.try_lock().map_err(|_| {
      napi::Error::from_reason("Failed to lock the bundler. Is another operation in progress?")
    })?;

    let binding_opts = resolve_output_options(opts)?;

    let maybe_outputs = bundler_core.generate(binding_opts).await;

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

    let output_chunks = outputs
      .into_iter()
      .map(|asset| OutputChunk { code: asset.content, file_name: asset.file_name })
      .collect::<Vec<_>>();
    Ok(output_chunks)
  }
}
