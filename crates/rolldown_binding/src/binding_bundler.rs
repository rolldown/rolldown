use std::sync::Arc;

use napi::Env;
use napi_derive::napi;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};

#[expect(unused)]
#[napi]
pub struct BindingBundler {
  session_id: Arc<str>,
  session_span: tracing::Span,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> napi::Result<Self> {
    let session_id: Arc<str> = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .expect("Time went backwards")
      .as_millis()
      .to_string()
      .into();

    let session_span = tracing::trace_span!("Session", session_id = &*session_id);

    Ok(Self { session_id, session_span })
  }

  #[napi]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn create_impl(
    &self,
    env: Env,
    options: BindingBundlerOptions,
  ) -> napi::Result<BindingBundlerImpl> {
    BindingBundlerImpl::new(env, options)
  }
}
