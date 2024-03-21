use napi::Env;
use rolldown_tracing::try_init_tracing_with_chrome_layer;
pub mod js_async_callback_ext;
pub mod normalize_binding_options;

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
