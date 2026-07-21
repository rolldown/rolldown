use napi_derive::napi;
use rolldown_dev::{
  BundleState, OnAdditionalAssetsCallback, OnHmrUpdatesCallback, OnOutputCallback,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::binding_dev_options::BindingDevOptions;
use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_client_hmr_update::BindingClientHmrUpdate;
use crate::types::binding_error_stage::BindingErrorStage;
use crate::types::binding_outputs::{BindingOutputs, to_binding_error};
use crate::types::error::{BindingErrors, BindingResult};
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};
use napi::bindgen_prelude::{FnArgs, PromiseRaw};
use napi::{Either, Env, threadsafe_function::ThreadsafeFunctionCallMode};

#[napi]
pub struct BindingDevEngine {
  inner: Arc<rolldown_dev::DevEngine>,
  cwd: Arc<Path>,
  _session_id: Arc<str>,
  _session: rolldown_devtools::Session,
}

#[napi]
impl BindingDevEngine {
  #[napi(constructor)]
  pub fn new(
    _env: Env,
    options: BindingBundlerOptions,
    dev_options: Option<BindingDevOptions>,
  ) -> napi::Result<Self> {
    let session_id = rolldown_devtools::generate_session_id();
    let session = rolldown_devtools::Session::dummy();

    let on_hmr_updates_callback = dev_options.as_ref().and_then(|opts| opts.on_hmr_updates.clone());
    let on_output_callback = dev_options.as_ref().and_then(|opts| opts.on_output.clone());
    let on_additional_assets_callback =
      dev_options.as_ref().and_then(|opts| opts.on_additional_assets.clone());

    let cwd: Arc<Path> = Arc::from(PathBuf::from(options.input_options.cwd.clone()));

    let rebuild_strategy =
      dev_options.as_ref().and_then(|opts| opts.rebuild_strategy).map(Into::into);
    // Take ownership of watch so we can consume Vec fields (include/exclude).
    let watch_options = dev_options.and_then(|opts| opts.watch);
    let skip_write = watch_options.as_ref().and_then(|watch| watch.skip_write);
    let use_polling = watch_options.as_ref().and_then(|watch| watch.use_polling);
    let poll_interval = watch_options.as_ref().and_then(|watch| watch.poll_interval);
    let use_debounce = watch_options.as_ref().and_then(|watch| watch.use_debounce);
    let debounce_duration = watch_options.as_ref().and_then(|watch| watch.debounce_duration);
    let compare_contents_for_polling =
      watch_options.as_ref().and_then(|watch| watch.compare_contents_for_polling);
    let debounce_tick_rate = watch_options.as_ref().and_then(|watch| watch.debounce_tick_rate);
    let (watch_include, watch_exclude) = watch_options
      .map(|watch| {
        let include = watch
          .include
          .map(crate::types::binding_string_or_regex::bindingify_string_or_regex_array);
        let exclude = watch
          .exclude
          .map(crate::types::binding_string_or_regex::bindingify_string_or_regex_array);
        (include, exclude)
      })
      .unwrap_or((None, None));

    // Create bundler config
    let bundler_config = create_bundler_config_from_binding_options(options)?;

    // If callback is provided, wrap it to convert BuildResult<(Vec<ClientHmrUpdate>, Vec<String>)> to BindingResult<(Vec<BindingClientHmrUpdate>, Vec<String>)>
    let on_hmr_updates = on_hmr_updates_callback.map(|js_callback| {
      let cwd = Arc::<Path>::clone(&cwd);
      Arc::new(
        move |result: rolldown_error::BuildResult<(
          Vec<rolldown_common::ClientHmrUpdate>,
          Vec<String>,
        )>| {
          let binding_result: BindingResult<(Vec<BindingClientHmrUpdate>, Vec<String>)> =
            match result {
              Ok((updates, changed_files)) => {
                let binding_updates: Vec<BindingClientHmrUpdate> =
                  updates.into_iter().map(BindingClientHmrUpdate::from).collect();
                Either::B((binding_updates, changed_files))
              }
              Err(errors) => {
                let binding_errors: Vec<_> = errors
                  .iter()
                  .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
                  .collect();
                Either::A(BindingErrors::new(binding_errors))
              }
            };
          js_callback
            .call(FnArgs { data: (binding_result,) }, ThreadsafeFunctionCallMode::Blocking);
        },
      ) as OnHmrUpdatesCallback
    });

    // If callback is provided, wrap it to convert BuildResult<BundleOutput> to BindingResult<BindingOutputs>
    let on_output = on_output_callback.map(|js_callback| {
      let cwd = Arc::<Path>::clone(&cwd);
      Arc::new(move |result: rolldown_error::BuildResult<rolldown::BundleOutput>| {
        let binding_result: BindingResult<BindingOutputs> = match result {
          Ok(bundle_output) => Either::B(BindingOutputs::from(bundle_output.assets)),
          Err(errors) => {
            let binding_errors: Vec<_> = errors
              .iter()
              .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
              .collect();
            Either::A(BindingErrors::new(binding_errors))
          }
        };
        js_callback.call(FnArgs { data: (binding_result,) }, ThreadsafeFunctionCallMode::Blocking);
      }) as OnOutputCallback
    });

    // Assets emitted during an HMR patch / lazy compile (these never go through
    // `on_output`). Forward the assets; warnings stay Rust-side, as in `on_output`.
    let on_additional_assets = on_additional_assets_callback.map(|js_callback| {
      Arc::new(move |output: rolldown::BundleOutput| {
        let binding_outputs = BindingOutputs::from(output.assets);
        js_callback.call(FnArgs { data: (binding_outputs,) }, ThreadsafeFunctionCallMode::Blocking);
      }) as OnAdditionalAssetsCallback
    });

    let dev_watch_options = if skip_write.is_some()
      || use_polling.is_some()
      || poll_interval.is_some()
      || use_debounce.is_some()
      || debounce_duration.is_some()
      || compare_contents_for_polling.is_some()
      || debounce_tick_rate.is_some()
      || watch_include.is_some()
      || watch_exclude.is_some()
    {
      Some(rolldown_dev::DevWatchOptions {
        disable_watcher: None,
        skip_write,
        use_polling,
        poll_interval: poll_interval.map(u64::from),
        use_debounce,
        debounce_duration: debounce_duration.map(u64::from),
        compare_contents_for_polling,
        debounce_tick_rate: debounce_tick_rate.map(u64::from),
        include: watch_include,
        exclude: watch_exclude,
      })
    } else {
      None
    };

    let rolldown_dev_options = rolldown_dev::DevOptions {
      on_hmr_updates,
      on_output,
      on_additional_assets,
      rebuild_strategy,
      watch: dev_watch_options,
    };

    let inner = rolldown_dev::DevEngine::new(bundler_config, rolldown_dev_options)
      .map_err(|e| napi::Error::from_reason(format!("Fail to create dev engine: {e:#?}")))?;

    Ok(Self { inner: Arc::new(inner), cwd, _session_id: session_id, _session: session })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn run<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.run().await.map_err(|_e| napi::Error::from_reason("Failed to run dev engine"))?;
      Ok(())
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn ensure_current_build_finish<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner
        .wait_for_ongoing_bundle()
        .await
        .map_err(|_e| napi::Error::from_reason("Failed to ensure current build finish"))?;
      Ok(())
    })
  }

  #[napi]
  pub fn get_bundle_state<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingBundleState>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner
        .get_bundle_state()
        .await
        .map(Into::into)
        .map_err(|_e| napi::Error::from_reason("Failed to get bundle state"))
    })
  }

  #[napi]
  pub fn ensure_latest_build_output<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      match inner.ensure_latest_bundle_output().await {
        Ok(()) => Ok(Either::B(())),
        Err(errors) => {
          let binding_errors: Vec<_> = errors
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
            .collect();
          Ok(Either::A(BindingErrors::new(binding_errors)))
        }
      }
    })
  }

  #[napi]
  pub fn trigger_full_build(&self) {
    self.inner.trigger_full_build().expect("Should handle this error");
  }

  /// Client-connect signal (the clientId hello): creates the per-client session
  /// with an empty ship map. Reconnects arrive as fresh clientIds.
  #[napi(ts_return_type = "Promise<void>")]
  pub fn register_client<'env>(
    &self,
    env: &'env Env,
    client_id: String,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.register_client(client_id).await;
      Ok(())
    })
  }

  /// Delivery notification from the serving middleware: the response for
  /// `filename` completed, so record its modules as shipped to that client.
  #[napi(ts_return_type = "Promise<void>")]
  pub fn notify_payload_delivered<'env>(
    &self,
    env: &'env Env,
    filename: String,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.notify_payload_delivered(&filename).await;
      Ok(())
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn remove_client<'env>(
    &self,
    env: &'env Env,
    client_id: String,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.remove_client(&client_id).await;
      Ok(())
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.close().await.map_err(|_e| napi::Error::from_reason("Failed to close dev engine"))?;
      Ok(())
    })
  }

  /// Compile a lazy entry module and return HMR-style patch code.
  ///
  /// This is called when a dynamically imported module is first requested at runtime.
  /// The module was previously stubbed with a proxy, and now we need to compile the
  /// actual module and its dependencies.
  #[napi]
  pub fn compile_entry<'env>(
    &self,
    env: &'env Env,
    module_id: String,
    client_id: String,
  ) -> napi::Result<PromiseRaw<'env, BindingLazyChunkOutput>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner
        .compile_lazy_entry(module_id, client_id)
        .await
        .map(|output| BindingLazyChunkOutput { code: output.code, filename: output.filename })
        .map_err(|e| napi::Error::from_reason(format!("Failed to compile lazy entry: {e:#?}")))
    })
  }
}

/// The client-facing slice of a lazy-compile result. The carried modules and
/// stamps stay server-side as the engine's pending-payload entry.
#[napi(object)]
pub struct BindingLazyChunkOutput {
  pub code: String,
  pub filename: String,
}

#[napi(object)]
pub struct BindingBundleState {
  pub last_build_errored: bool,
  /// The stage of the last incremental failure, when `last_build_errored`
  /// is true and the engine is in an incremental-failure state. Absent on
  /// success and for an initial full-build failure (use
  /// `last_build_errored` to detect that). The consumer can force a full
  /// rebuild on the next page load when this is `Hmr`. See
  /// `internal-docs/dev-engine/implementation.md` §12.
  pub last_error_stage: Option<BindingErrorStage>,
  pub has_stale_output: bool,
}

impl From<BundleState> for BindingBundleState {
  fn from(state: BundleState) -> Self {
    Self {
      last_build_errored: state.last_build_errored,
      last_error_stage: state.last_error_stage.map(Into::into),
      has_stale_output: state.has_stale_output,
    }
  }
}
