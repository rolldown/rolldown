use napi_derive::napi;
use std::sync::Arc;

use napi::{Env, threadsafe_function::ThreadsafeFunctionCallMode};
use rolldown_watcher::NotifyWatcher;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};
use crate::binding_dev_options::BindingDevOptions;
use crate::types::binding_hmr_output::BindingHmrUpdate;
use crate::types::js_callback::JsCallback;
use napi::bindgen_prelude::FnArgs;

#[napi]
pub struct BindingDevEngine {
  inner: rolldown::DevEngine<NotifyWatcher>,
  _session_id: Arc<str>,
  _session: rolldown_debug::Session,
  #[allow(dead_code)]
  on_hmr_updates: Option<JsCallback<FnArgs<(Vec<BindingHmrUpdate>,)>, ()>>,
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

    let on_hmr_updates_callback = dev_options.and_then(|opts| opts.on_hmr_updates);

    // Create rolldown DevOptions
    let mut rolldown_dev_options = rolldown::dev::dev_options::DevOptions::default();

    // If callback is provided, wrap it to convert Vec<HmrUpdate> to Vec<BindingHmrUpdate>
    if let Some(js_callback) = on_hmr_updates_callback.clone() {
      let callback = Arc::new(move |updates: Vec<rolldown_common::HmrUpdate>| {
        let binding_updates: Vec<BindingHmrUpdate> =
          updates.into_iter().map(BindingHmrUpdate::from).collect();
        js_callback.call(FnArgs { data: (binding_updates,) }, ThreadsafeFunctionCallMode::Blocking);
      });
      rolldown_dev_options.on_hmr_updates = Some(callback);
    }

    let inner = rolldown::DevEngine::with_bundler(bundler, rolldown_dev_options)
      .map_err(|_e| napi::Error::from_reason("Fail to create dev engine"))?;

    Ok(Self {
      inner,
      _session_id: session_id,
      _session: session,
      on_hmr_updates: on_hmr_updates_callback,
    })
  }

  #[napi]
  pub async fn run(&self) -> napi::Result<()> {
    self.inner.run().await;
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
  ) -> napi::Result<()> {
    self.inner.invalidate(caller, first_invalidated_by).await.expect("Should handle this error");
    Ok(())
  }
}
