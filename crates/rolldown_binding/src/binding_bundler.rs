use std::sync::Arc;

use napi::Env;
use napi_derive::napi;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};

#[napi]
pub struct BindingBundler {
  session_id: Arc<str>,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> napi::Result<Self> {
    let session_id = rolldown_debug::generate_session_id();

    Ok(Self { session_id })
  }

  #[napi]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn create_impl(
    &self,
    env: Env,
    options: BindingBundlerOptions,
  ) -> napi::Result<BindingBundlerImpl> {
    BindingBundlerImpl::new(env, options, Arc::clone(&self.session_id))
  }
}
