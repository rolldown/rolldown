use std::sync::{
  Arc,
  atomic::{AtomicI64, Ordering},
};

#[cfg(not(target_family = "wasm"))]
use crate::worker_manager::WorkerManager;
use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
  types::{
    binding_outputs::{BindingOutputs, to_binding_error},
    error::{BindingError, BindingErrors, BindingResult},
  },
  utils::normalize_binding_options::normalize_binding_options,
};
use napi::{
  Env,
  bindgen_prelude::{ObjectFinalize, PromiseRaw},
  tokio::sync::Mutex,
};
use napi_derive::napi;
use rolldown::{Bundler as NativeBundler, BundlerBuilder, LogLevel, NormalizedBundlerOptions};
use rolldown_common::ScanMode;
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosticOptions, SingleBuildResult,
  filter_out_disabled_diagnostics,
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingBundlerOptions<'env> {
  pub input_options: BindingInputOptions<'env>,
  pub output_options: BindingOutputOptions<'env>,
  pub parallel_plugins_registry: Option<ParallelJsPluginRegistry>,
}

#[napi(custom_finalize)]
pub struct BindingBundlerImpl {
  inner: Arc<Mutex<NativeBundler>>,
  memory_adjustment: Arc<AtomicI64>,
}

impl ObjectFinalize for BindingBundlerImpl {
  fn finalize(self, env: Env) -> napi::Result<()> {
    let memory_adjustment = self.memory_adjustment.load(Ordering::Relaxed);
    if memory_adjustment > 0 {
      env.adjust_external_memory(-memory_adjustment)?;
    }
    Ok(())
  }
}

#[napi]
impl BindingBundlerImpl {
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn new(
    option: BindingBundlerOptions,
    session: rolldown_debug::Session,
    build_count: u32,
  ) -> napi::Result<Self> {
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

    let bundler_builder = BundlerBuilder::default()
      .with_options(ret.bundler_options)
      .with_plugins(ret.plugins)
      .with_build_count(build_count)
      .with_session(session)
      .with_disable_tracing_setup(true);

    // TODO: improve the following error message
    let bundler = bundler_builder.build().map_err(|err| {
      napi::Error::new(
        napi::Status::GenericFailure,
        err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
      )
    })?;

    Ok(Self {
      inner: Arc::new(Mutex::new(bundler)),
      memory_adjustment: Arc::new(AtomicI64::new(0)),
    })
  }

  pub fn new_with_bundler(inner: Arc<Mutex<NativeBundler>>) -> Self {
    Self { inner, memory_adjustment: Arc::new(AtomicI64::new(0)) }
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn write<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let inner = Arc::clone(&self.inner);
    let memory_adjustment = Arc::clone(&self.memory_adjustment);

    env.spawn_future_with_callback(
      async move {
        let outputs = Self::write_impl(inner).await?;
        Ok(outputs)
      },
      move |env, outputs| {
        if let napi::Either::B(ref binding_outputs) = outputs {
          let chunk_size = binding_outputs.chunk_len();
          // 16mb per chunk, it's just a hint, so it doesn't need to be accurate
          #[expect(clippy::cast_possible_wrap)]
          let memory_consumption = chunk_size as i64 * 1024 * 1024 * 16;
          memory_adjustment.fetch_add(memory_consumption, Ordering::Relaxed);
          env.adjust_external_memory(memory_consumption)?;
        }
        Ok(outputs)
      },
    )
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(&self) -> napi::Result<BindingResult<BindingOutputs>> {
    self.generate_impl().await
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(&self) -> napi::Result<BindingResult<BindingOutputs>> {
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
    Ok(napi::bindgen_prelude::block_on(async { self.inner.lock().await.closed }))
  }

  #[napi]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn get_watch_files(&self) -> napi::Result<Vec<String>> {
    let bundler_core = self.inner.lock().await;
    Ok(bundler_core.get_watch_files().iter().map(|s| s.to_string()).collect())
  }
}

impl BindingBundlerImpl {
  #[expect(clippy::significant_drop_tightening)]
  pub async fn scan_impl(&self) -> napi::Result<BindingResult<BindingOutputs>> {
    let mut bundler_core = self.inner.lock().await;
    let output =
      Self::handle_result(bundler_core.scan(ScanMode::Full).await, bundler_core.options());

    match output {
      Ok(output) => {
        if let Err(err) = Self::handle_warnings(output.warnings, bundler_core.options()).await {
          let error = to_binding_error(&err, bundler_core.options().cwd.clone());
          return Ok(napi::Either::A(BindingErrors::new(vec![error])));
        }
      }
      Err(errors) => {
        return Ok(napi::Either::A(BindingErrors::new(errors)));
      }
    }

    Ok(napi::Either::B(vec![].into()))
  }

  #[expect(clippy::significant_drop_tightening)]
  pub async fn write_impl(
    bundler: Arc<Mutex<NativeBundler>>,
  ) -> napi::Result<BindingResult<BindingOutputs>> {
    let mut bundler_core = bundler.lock().await;

    let outputs = match bundler_core.write().await {
      Ok(outputs) => outputs,
      Err(errs) => {
        let errors: Vec<BindingError> = errs
          .into_vec()
          .iter()
          .map(|diagnostic| to_binding_error(diagnostic, bundler_core.options().cwd.clone()))
          .collect();
        return Ok(napi::Either::A(BindingErrors::new(errors)));
      }
    };

    if let Err(err) = Self::handle_warnings(outputs.warnings, bundler_core.options()).await {
      let error = to_binding_error(&err, bundler_core.options().cwd.clone());
      return Ok(napi::Either::A(BindingErrors::new(vec![error])));
    }

    Ok(napi::Either::B(outputs.assets.into()))
  }

  #[expect(clippy::significant_drop_tightening)]
  pub async fn generate_impl(&self) -> napi::Result<BindingResult<BindingOutputs>> {
    let mut bundler_core = self.inner.lock().await;

    let bundle_output = match bundler_core.generate().await {
      Ok(output) => output,
      Err(errs) => {
        let errors: Vec<BindingError> = errs
          .into_vec()
          .iter()
          .map(|diagnostic| to_binding_error(diagnostic, bundler_core.options().cwd.clone()))
          .collect();
        return Ok(napi::Either::A(BindingErrors::new(errors)));
      }
    };

    if let Err(err) = Self::handle_warnings(bundle_output.warnings, bundler_core.options()).await {
      let error = to_binding_error(&err, bundler_core.options().cwd.clone());
      return Ok(napi::Either::A(BindingErrors::new(vec![error])));
    }

    Ok(napi::Either::B(bundle_output.assets.into()))
  }

  pub async fn close_impl(&self) -> napi::Result<()> {
    let mut bundler_core = self.inner.lock().await;
    Ok(bundler_core.close().await?)
  }

  pub fn into_inner(self) -> Arc<Mutex<NativeBundler>> {
    self.inner
  }

  fn handle_result<T>(
    result: BuildResult<T>,
    options: &NormalizedBundlerOptions,
  ) -> Result<T, Vec<crate::types::error::BindingError>> {
    result.map_err(|e| {
      e.into_vec()
        .iter()
        .map(|diagnostic| to_binding_error(diagnostic, options.cwd.clone()))
        .collect()
    })
  }

  async fn handle_warnings(
    warnings: Vec<BuildDiagnostic>,
    options: &NormalizedBundlerOptions,
  ) -> SingleBuildResult<()> {
    if options.log_level == Some(LogLevel::Silent) {
      return Ok(());
    }
    if let Some(on_log) = options.on_log.as_ref() {
      for warning in filter_out_disabled_diagnostics(warnings, &options.checks) {
        on_log
          .call(
            LogLevel::Warn,
            rolldown::Log {
              id: warning.id(),
              exporter: warning.exporter(),
              code: Some(warning.kind().to_string()),
              message: warning
                .to_diagnostic_with(&DiagnosticOptions { cwd: options.cwd.clone() })
                .to_color_string(),
              plugin: None,
            },
          )
          .await?;
      }
    }
    Ok(())
  }
}
