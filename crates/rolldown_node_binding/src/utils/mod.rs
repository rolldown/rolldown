pub mod napi_error_ext;
use std::sync::atomic::AtomicBool;

use napi::Env;
mod into_js_unknown_vec;
pub use into_js_unknown_vec::*;
mod js_callback;
pub use js_callback::*;
use rolldown_tracing::enable_tracing_on_demand;

static IS_ENABLE_TRACING: AtomicBool = AtomicBool::new(false);

pub fn init_custom_trace_subscriber(mut env: Env) {
  if !IS_ENABLE_TRACING.swap(true, std::sync::atomic::Ordering::SeqCst) {
    let guard = enable_tracing_on_demand();
    if let Some(guard) = guard {
      env
        .add_env_cleanup_hook(guard, |flush_guard| {
          flush_guard.flush();
          drop(flush_guard);
        })
        .expect("Should able to initialize cleanup for custom trace subscriber");
    }
  }
}
