use napi::tokio;
use napi_derive::napi;
use rolldown_dev::{BundleState, BundlingFuture, OnHmrUpdatesCallback, OnOutputCallback};
use std::path::PathBuf;
use std::sync::Arc;

use crate::binding_dev_options::BindingDevOptions;
use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_client_hmr_update::BindingClientHmrUpdate;
use crate::types::binding_outputs::{BindingOutputs, to_binding_error};
use crate::types::error::{BindingErrors, BindingResult};
use crate::utils::create_bundler_from_binding_options::create_bundler_from_binding_options;
use napi::bindgen_prelude::FnArgs;
use napi::{Either, Env, threadsafe_function::ThreadsafeFunctionCallMode};

#[napi]
pub struct BindingDevEngine {
  inner: rolldown_dev::DevEngine,
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

    let cwd = Arc::new(PathBuf::from(options.input_options.cwd.clone()));

    let rebuild_strategy =
      dev_options.as_ref().and_then(|opts| opts.rebuild_strategy).map(Into::into);
    let watch_options = dev_options.as_ref().and_then(|opts| opts.watch.as_ref());
    let skip_write = watch_options.and_then(|watch| watch.skip_write);
    let use_polling = watch_options.and_then(|watch| watch.use_polling);
    let poll_interval = watch_options.and_then(|watch| watch.poll_interval);
    let use_debounce = watch_options.and_then(|watch| watch.use_debounce);
    let debounce_duration = watch_options.and_then(|watch| watch.debounce_duration);
    let compare_contents_for_polling =
      watch_options.and_then(|watch| watch.compare_contents_for_polling);
    let debounce_tick_rate = watch_options.and_then(|watch| watch.debounce_tick_rate);

    // Create bundler
    let bundler: Arc<napi::tokio::sync::Mutex<rolldown::Bundler>> =
      Arc::new(tokio::sync::Mutex::new(create_bundler_from_binding_options(options)?));

    // If callback is provided, wrap it to convert BuildResult<(Vec<ClientHmrUpdate>, Vec<String>)> to BindingResult<(Vec<BindingClientHmrUpdate>, Vec<String>)>
    let on_hmr_updates = on_hmr_updates_callback.map(|js_callback| {
      let cwd = Arc::<std::path::PathBuf>::clone(&cwd);
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
      let cwd = Arc::<std::path::PathBuf>::clone(&cwd);
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

    let dev_watch_options = if skip_write.is_some()
      || use_polling.is_some()
      || poll_interval.is_some()
      || use_debounce.is_some()
      || debounce_duration.is_some()
      || compare_contents_for_polling.is_some()
      || debounce_tick_rate.is_some()
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
      })
    } else {
      None
    };

    let rolldown_dev_options = rolldown_dev::DevOptions {
      on_hmr_updates,
      on_output,
      rebuild_strategy,
      watch: dev_watch_options,
    };

    let inner = rolldown_dev::DevEngine::with_bundler(bundler, rolldown_dev_options)
      .map_err(|e| napi::Error::from_reason(format!("Fail to create dev engine: {e:#?}")))?;

    Ok(Self { inner, _session_id: session_id, _session: session })
  }

  #[napi]
  pub async fn run(&self) -> napi::Result<()> {
    self.inner.run().await.map_err(|_e| napi::Error::from_reason("Failed to run dev engine"))?;
    Ok(())
  }

  #[napi]
  pub async fn ensure_current_build_finish(&self) -> napi::Result<()> {
    self
      .inner
      .wait_for_ongoing_bundle()
      .await
      .map_err(|_e| napi::Error::from_reason("Failed to ensure current build finish"))?;
    Ok(())
  }

  #[napi]
  pub async fn get_bundle_state(&self) -> napi::Result<BindingBundleState> {
    self
      .inner
      .get_bundle_state()
      .await
      .map(Into::into)
      .map_err(|_e| napi::Error::from_reason("Failed to get bundle state"))
  }

  #[napi]
  pub async fn ensure_latest_build_output(&self) -> napi::Result<()> {
    self.inner.ensure_latest_bundle_output().await.expect("Should handle this error");
    Ok(())
  }

  #[napi]
  pub async fn invalidate(
    &self,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> napi::Result<Vec<BindingClientHmrUpdate>> {
    let updates =
      self.inner.invalidate(caller, first_invalidated_by).await.expect("Should handle this error");
    let binding_updates = updates.into_iter().map(BindingClientHmrUpdate::from).collect();
    Ok(binding_updates)
  }

  #[napi]
  #[allow(
    clippy::unused_async,
    clippy::allow_attributes,
    reason = "`.entry()` is acquiring a lock. Making this async to avoid blocking the nodejs thread or cause deadlock if lock is contended."
  )]
  pub async fn register_modules(&self, client_id: String, modules: Vec<String>) {
    self.inner.clients.entry(client_id).or_default().executed_modules.extend(modules);
  }

  #[napi]
  pub fn remove_client(&self, client_id: String) {
    self.inner.clients.remove(&client_id);
  }

  #[napi]
  pub async fn close(&self) -> napi::Result<()> {
    self
      .inner
      .close()
      .await
      .map_err(|_e| napi::Error::from_reason("Failed to close dev engine"))?;
    Ok(())
  }
}

#[napi]
pub struct ScheduledBuild {
  future: BundlingFuture,
  already_scheduled: bool,
}

#[napi]
impl ScheduledBuild {
  #[napi]
  pub async fn wait(&self) -> napi::Result<()> {
    self.future.clone().await;
    Ok(())
  }

  #[napi]
  pub fn already_scheduled(&self) -> bool {
    self.already_scheduled
  }
}

#[napi(object)]
pub struct BindingBundleState {
  pub last_full_build_failed: bool,
  pub has_stale_output: bool,
}

impl From<BundleState> for BindingBundleState {
  fn from(state: BundleState) -> Self {
    Self {
      last_full_build_failed: state.last_full_build_failed,
      has_stale_output: state.has_stale_output,
    }
  }
}
