pub mod globs;
mod into_js_unknown_vec;
mod js_callback;
pub mod napi_error_ext;

pub use into_js_unknown_vec::*;
pub use js_callback::*;
use napi::Env;
use rolldown_tracing::try_init_tracing_with_chrome_layer;

pub fn try_init_custom_trace_subscriber(mut napi_env: Env) {
  match std::env::var("LOG_LAYER") {
    Ok(val) if val == "chrome" => {
      let guard = try_init_tracing_with_chrome_layer();
      if let Some(guard) = guard {
        napi_env
          .add_env_cleanup_hook(guard, |flush_guard| {
            flush_guard.flush();
            drop(flush_guard);
          })
          .expect("Should able to initialize cleanup for custom trace subscriber");
      }
    }
    _ => {}
  }
}
