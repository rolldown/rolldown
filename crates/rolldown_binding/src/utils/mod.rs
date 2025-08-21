use napi::Env;
use rolldown_tracing::try_init_tracing;
pub mod minify_options_conversion;
pub mod napi_error;
pub mod normalize_binding_options;

pub fn try_init_custom_trace_subscriber(napi_env: Env) {
  let maybe_guard = try_init_tracing();
  if let Some(guard) = maybe_guard {
    napi_env
      .add_env_cleanup_hook(guard, |flush_guard| {
        // flush_guard.flush();
        drop(flush_guard);
      })
      .expect("Should able to initialize cleanup for custom trace subscriber");
  }
}

pub fn handle_result<T>(result: anyhow::Result<T>) -> napi::Result<T> {
  result.map_err(|e| match e.downcast::<napi::Error>() {
    Ok(e) => e,
    Err(e) => napi::Error::from_reason(format!("Rolldown internal error: {e}")),
  })
}
