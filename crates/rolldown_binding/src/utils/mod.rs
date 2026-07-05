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
use rolldown::{LogLevel, NormalizedBundlerOptions};
use rolldown_error::{
  BuildDiagnostic, Diagnostic, DiagnosticOptions, filter_out_disabled_diagnostics,
};
use rolldown_tracing::try_init_tracing;

pub use normalize_binding_transform_options::normalize_binding_transform_options;

/// Box a future before handing it to napi's [`Env::spawn_future`].
///
/// `Env::spawn_future` is monomorphized over the concrete future type, so every
/// distinct async body re-instantiates the tokio task harness (`poll_future`,
/// `Core<T, S>`, the scheduler dispatch, ...). Binding entry points run once per
/// operation, so erasing the future to a single `Pin<Box<dyn Future>>` collapses
/// that machinery to one instantiation per output type, at the cost of one
/// heap allocation per call. Do not use this for per-module/per-hook futures,
/// where the extra allocation would be on a hot path.
pub fn spawn_boxed_future<T: 'static + Send + ToNapiValue>(
  env: &Env,
  fut: impl 'static + Send + Future<Output = napi::Result<T>>,
) -> napi::Result<PromiseRaw<'_, T>> {
  let fut: Pin<Box<dyn Future<Output = napi::Result<T>> + Send>> = Box::pin(fut);
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
    Err(e) => {
      // Replayable lifecycle futures retain failures behind a shared wrapper.
      // Clone a nested napi error so the original JS exception object, stack,
      // subclass, and own properties survive the Rust close state machine.
      if let Some(error) = e.chain().find_map(|cause| cause.downcast_ref::<napi::Error>()) {
        return error.try_clone().unwrap_or_else(|clone_error| clone_error);
      }
      napi::Error::from_reason(format!("Rolldown internal error: {e}"))
    }
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
  let rendered = Diagnostic::render_batch(&diagnostics, true);

  // Dispatch the callbacks sequentially, awaiting each before invoking the next.
  // A warning handler is allowed to `throw` to abort the build, so we must stop at
  // the first failure without invoking any later handler (and without leaving
  // concurrent in-flight calls racing the returned error). The expensive part of
  // #9748 was the per-warning re-render above, not the JS round-trip, so awaiting
  // one call at a time does not reintroduce the hang.
  for (warning, rendered) in warnings.into_iter().zip(rendered) {
    // Only include loc/pos for warning types that report specific source locations.
    // Use warning.id() for the file path since the diagnostic may only store the filename.
    #[expect(
      clippy::cast_possible_truncation,
      reason = "line/column/position values are unlikely to exceed u32::MAX in practical use"
    )]
    let (loc, pos) = match rendered.primary_location {
      Some(location) => (
        Some(rolldown::LogLocation {
          line: location.line as u32,
          column: location.column as u32,
          file: warning.id(),
        }),
        Some(location.utf16_position as u32),
      ),
      None => (None, None),
    };

    on_log
      .call(
        LogLevel::Warn,
        rolldown::Log {
          id: warning.id(),
          exporter: warning.exporter(),
          code: Some(warning.kind().to_string()),
          message: rendered.message,
          plugin: warning.plugin(),
          loc,
          pos,
          ids: warning.ids(),
        },
      )
      .await?;
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::{fmt, sync::Arc};

  use super::handle_result;

  #[derive(Debug)]
  struct SharedNapiError(Arc<napi::Error>);

  impl fmt::Display for SharedNapiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      self.0.fmt(f)
    }
  }

  impl std::error::Error for SharedNapiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      Some(self.0.as_ref())
    }
  }

  #[test]
  fn handle_result_preserves_nested_napi_errors() {
    let error =
      anyhow::Error::new(SharedNapiError(Arc::new(napi::Error::from_reason("close bundle error"))));
    let error = handle_result::<()>(Err(error)).expect_err("nested napi error must be returned");
    assert_eq!(error.reason, "close bundle error");
  }
}
