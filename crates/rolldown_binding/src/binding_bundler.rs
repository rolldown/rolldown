use std::sync::Arc;

use napi::Env;
use napi_derive::napi;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};

#[napi]
pub struct BindingBundler {
  session_id: Arc<str>,
  // Every `.write(..)/.generate(..)` will create a new `BindingBundlerImpl`, we use this field to track the build count.
  build_count: u32,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> napi::Result<Self> {
    let session_id = rolldown_debug::generate_session_id();
    let build_count = 0;

    Ok(Self { session_id, build_count })
  }

  #[napi]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn create_impl(
    &mut self,
    env: Env,
    options: BindingBundlerOptions,
  ) -> napi::Result<BindingBundlerImpl> {
    self.build_count += 1;
    BindingBundlerImpl::new(env, options, Arc::clone(&self.session_id), self.build_count)
  }
}
