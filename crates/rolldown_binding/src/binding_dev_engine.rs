use napi_derive::napi;
use rolldown::dev::OnHmrUpdatesCallback;
use std::sync::Arc;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};
use crate::binding_dev_options::BindingDevOptions;
use crate::types::binding_hmr_output::BindingHmrUpdate;
use napi::bindgen_prelude::FnArgs;
use napi::{Env, threadsafe_function::ThreadsafeFunctionCallMode};

#[napi]
pub struct BindingDevEngine {
  inner: rolldown::DevEngine,
  _session_id: Arc<str>,
  _session: rolldown_debug::Session,
}

#[napi]
impl BindingDevEngine {
  #[napi(constructor)]
  pub fn new(
    _env: Env,
    options: BindingBundlerOptions,
    dev_options: Option<BindingDevOptions>,
  ) -> napi::Result<Self> {
    let bundler =
      BindingBundlerImpl::new(options, rolldown_debug::Session::dummy(), 0)?.into_inner();

    let session_id = rolldown_debug::generate_session_id();
    let session = rolldown_debug::Session::dummy();

    let on_hmr_updates_callback = dev_options.as_ref().and_then(|opts| opts.on_hmr_updates.clone());
    let watch_options = dev_options.as_ref().and_then(|opts| opts.watch.as_ref());
    let use_polling = watch_options.and_then(|watch| watch.use_polling);
    let poll_interval = watch_options.and_then(|watch| watch.poll_interval);
    let use_debounce = watch_options.and_then(|watch| watch.use_debounce);
    let debounce_duration = watch_options.and_then(|watch| watch.debounce_duration);

    // If callback is provided, wrap it to convert Vec<HmrUpdate> to Vec<BindingHmrUpdate>
    let on_hmr_updates = on_hmr_updates_callback.map(|js_callback| {
      Arc::new(move |updates: Vec<rolldown_common::HmrUpdate>| {
        let binding_updates: Vec<BindingHmrUpdate> =
          updates.into_iter().map(BindingHmrUpdate::from).collect();
        js_callback.call(FnArgs { data: (binding_updates,) }, ThreadsafeFunctionCallMode::Blocking);
      }) as OnHmrUpdatesCallback
    });

    let dev_watch_options = if use_polling.is_some()
      || poll_interval.is_some()
      || use_debounce.is_some()
      || debounce_duration.is_some()
    {
      Some(rolldown::dev::dev_options::DevWatchOptions {
        use_polling,
        poll_interval: poll_interval.map(u64::from),
        use_debounce,
        debounce_duration: debounce_duration.map(u64::from),
      })
    } else {
      None
    };

    let rolldown_dev_options = rolldown::dev::dev_options::DevOptions {
      on_hmr_updates,
      watch: dev_watch_options,
      ..Default::default()
    };

    let inner = rolldown::DevEngine::with_bundler(bundler, rolldown_dev_options)
      .map_err(|_e| napi::Error::from_reason("Fail to create dev engine"))?;

    Ok(Self { inner, _session_id: session_id, _session: session })
  }

  #[napi]
  pub async fn run(&self) -> napi::Result<()> {
    self.inner.run().await.map_err(|_e| napi::Error::from_reason("Failed to run dev engine"))?;
    Ok(())
  }

  #[napi]
  pub async fn ensure_current_build_finish(&self) -> napi::Result<()> {
    self.inner.ensure_current_build_finish().await;
    Ok(())
  }

  #[napi]
  pub async fn ensure_latest_build(&self) -> napi::Result<()> {
    self.inner.ensure_latest_build().await.expect("Should handle this error");
    Ok(())
  }

  #[napi]
  pub async fn invalidate(
    &self,
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> napi::Result<BindingHmrUpdate> {
    let update =
      self.inner.invalidate(caller, first_invalidated_by).await.expect("Should handle this error");
    Ok(BindingHmrUpdate::from(update))
  }
}
