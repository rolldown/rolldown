mod normalize_binding_transform_options;

pub mod collapse_sourcemaps;
pub mod create_bundler_config_from_binding_options;
pub mod minify_options_conversion;
pub mod napi_error;
pub mod normalize_binding_options;

use std::any::Any;
use std::{future::Future, pin::Pin};

use napi::{
  Env,
  bindgen_prelude::{PromiseRaw, ToNapiValue},
};
use napi_derive::napi;
use rolldown::{Log, LogLevel, NormalizedBundlerOptions};
use rolldown_error::{
  BuildDiagnostic, Diagnostic, DiagnosticOptions, filter_out_disabled_diagnostics,
};
use rolldown_tracing::try_init_tracing;

pub use normalize_binding_transform_options::normalize_binding_transform_options;

/// Spawn the future of an async binding method and return its promise.
///
/// Boxing gives every async body one tokio task harness per output type, at one heap allocation
/// per call.
/// On wasm, the call also holds the shared runtime until its promise settles, so every async
/// binding method that spawns, including the per-hook `this.resolve()` and `this.load()`, goes
/// through here.
pub fn spawn_boxed_future<T: 'static + Send + ToNapiValue>(
  env: &Env,
  fut: impl 'static + Send + Future<Output = napi::Result<T>>,
) -> napi::Result<PromiseRaw<'_, T>> {
  let fut: Pin<Box<dyn Future<Output = napi::Result<T>> + Send>> = Box::pin(fut);
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    // Calls retain the runtime until their JS promise settles.
    // See internal-docs/rust-classic-bundler/implementation.md.
    crate::start_async_runtime();
    let result = napi::bindgen_prelude::execute_tokio_future_with_finalize_callback(
      env.raw(),
      fut,
      |env, value| unsafe { T::to_napi_value(env, value) },
      Some(Box::new(|_| crate::shutdown_async_runtime())),
    );
    match result {
      Ok(promise) => Ok(PromiseRaw::new(env.raw(), promise)),
      Err(error) => {
        crate::shutdown_async_runtime();
        Err(error)
      }
    }
  }
  #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
  env.spawn_future(fut)
}

#[napi]
pub struct TraceSubscriberGuard {
  guard: Option<Box<dyn Any + Send>>,
}

#[napi]
impl TraceSubscriberGuard {
  #[napi]
  pub fn close(&mut self) {
    self.guard.take();
  }
}

#[napi]
pub fn init_trace_subscriber() -> Option<TraceSubscriberGuard> {
  let maybe_guard = try_init_tracing();
  let guard = maybe_guard?;
  Some(TraceSubscriberGuard { guard: Some(guard) })
}

pub fn handle_result<T>(result: anyhow::Result<T>) -> napi::Result<T> {
  result.map_err(|e| match e.downcast::<napi::Error>() {
    Ok(e) => e,
    Err(e) => napi::Error::from_reason(format!("Rolldown internal error: {e}")),
  })
}

pub async fn handle_warnings(
  warnings: Vec<BuildDiagnostic>,
  options: &NormalizedBundlerOptions,
) -> anyhow::Result<()> {
  if options.log_level == Some(LogLevel::Silent) {
    return Ok(());
  }
  let Some(on_log) = options.on_log.as_ref() else {
    return Ok(());
  };

  let warnings: Vec<BuildDiagnostic> =
    filter_out_disabled_diagnostics(warnings, &options.checks).collect();
  if warnings.is_empty() {
    return Ok(());
  }

  // Render every warning up front through the batch API. Rendering per-warning
  // rebuilds the line index / ariadne `Source` for the whole file each time,
  // which is O(N^2) for many warnings in one large file and is what actually
  // makes a high-volume build appear to hang (#9748).
  let diagnostic_options = DiagnosticOptions { cwd: options.cwd.clone() };
  let diagnostics: Vec<Diagnostic> =
    warnings.iter().map(|warning| warning.to_diagnostic_with(&diagnostic_options)).collect();
  let rendered = Diagnostic::render_batch(&diagnostics, rolldown_error::stderr_supports_color());

  // Dispatch the callbacks sequentially, awaiting each before invoking the next.
  // A warning handler is allowed to `throw` to abort the build, so we must stop at
  // the first failure without invoking any later handler (and without leaving
  // concurrent in-flight calls racing the returned error). The expensive part of
  // #9748 was the per-warning re-render above, not the JS round-trip, so awaiting
  // one call at a time does not reintroduce the hang.
  for (warning, rendered) in warnings.into_iter().zip(rendered) {
    on_log.call(LogLevel::Warn, Log::from_rendered(&warning, rendered)).await?;
  }

  Ok(())
}
