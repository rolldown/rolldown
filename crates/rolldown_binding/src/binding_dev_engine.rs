use napi_derive::napi;
use std::sync::Arc;

use napi::Env;
use rolldown_watcher::NotifyWatcher;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};

#[napi]

pub struct BindingDevEngine {
  inner: rolldown::DevEngine<NotifyWatcher>,
  _session_id: Arc<str>,
  _session: rolldown_debug::Session,
}

#[napi]
impl BindingDevEngine {
  #[napi(constructor)]
  pub fn new(env: Env, options: BindingBundlerOptions) -> napi::Result<Self> {
    let bundler =
      BindingBundlerImpl::new(env, options, rolldown_debug::Session::dummy(), 0)?.into_inner();

    let session_id = rolldown_debug::generate_session_id();
    let session = rolldown_debug::Session::dummy();

    let inner = rolldown::DevEngine::with_bundler(bundler)
      .map_err(|_e| napi::Error::from_reason("Fail to create dev engine"))?;

    Ok(Self { inner, _session_id: session_id, _session: session })
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
}
