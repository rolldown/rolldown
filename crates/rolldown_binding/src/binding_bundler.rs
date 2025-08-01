use napi::Env;
use napi_derive::napi;
use oxc::span::CompactStr;

use crate::binding_bundler_impl::{BindingBundlerImpl, BindingBundlerOptions};

#[napi]
pub struct BindingBundler {
  // Every `.write(..)/.generate(..)` will create a new `BindingBundlerImpl`, we use this field to track the build count.
  build_count: u32,
  session_id: CompactStr,
  debug_tracer: Option<rolldown_debug::DebugTracer>,
  session: rolldown_debug::Session,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> napi::Result<Self> {
    let session_id = rolldown_debug::generate_session_id();
    Ok(Self {
      session_id,
      build_count: 0,
      debug_tracer: None,
      session: rolldown_debug::Session::dummy(),
    })
  }

  #[napi]
  #[cfg_attr(target_family = "wasm", allow(unused))]
  pub fn create_impl(
    &mut self,
    env: Env,
    options: BindingBundlerOptions,
  ) -> napi::Result<BindingBundlerImpl> {
    if self.debug_tracer.is_none() && options.input_options.debug.is_some() {
      self.debug_tracer = Some(rolldown_debug::DebugTracer::init(self.session_id.clone()));
      // Caveat: `Span` must be created after initialization of `DebugTracer`, we need it to inject data to the tracking system.
      let session_span =
        tracing::debug_span!("session", CONTEXT_session_id = self.session_id.as_str());
      // Update the `session` with the actual session span
      self.session = rolldown_debug::Session::new(self.session_id.clone(), session_span);
    }

    self.build_count += 1;
    BindingBundlerImpl::new(env, options, self.session.clone(), self.build_count)
  }
}
