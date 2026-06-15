mod normalize_binding_transform_options;

pub mod collapse_sourcemaps;
pub mod create_bundler_config_from_binding_options;
pub mod minify_options_conversion;
pub mod napi_error;
pub mod normalize_binding_options;

use std::any::Any;

use futures::stream::{StreamExt, TryStreamExt};
use napi_derive::napi;
use rolldown::{LogLevel, NormalizedBundlerOptions};
use rolldown_error::{
  BuildDiagnostic, Diagnostic, DiagnosticOptions, filter_out_disabled_diagnostics,
};
use rolldown_tracing::try_init_tracing;

pub use normalize_binding_transform_options::normalize_binding_transform_options;

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
  // Emitting warnings one at a time forces a Rust -> JS -> Rust round-trip per
  // warning; pipelining the calls with bounded concurrency lets the JS event loop
  // drain many queued calls per tick while capping the number of in-flight calls
  // (and therefore memory). See #9748.
  const MAX_IN_FLIGHT_WARNINGS: usize = 256;

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

  let logs: Vec<rolldown::Log> = warnings
    .into_iter()
    .zip(rendered)
    .map(|(warning, rendered)| {
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

      rolldown::Log {
        id: warning.id(),
        exporter: warning.exporter(),
        code: Some(warning.kind().to_string()),
        message: rendered.message,
        plugin: warning.plugin(),
        loc,
        pos,
        ids: warning.ids(),
      }
    })
    .collect();

  // `buffer_unordered` is fine here: warnings are collected from parallel module
  // tasks in completion order, so there is no cross-module ordering to preserve.
  futures::stream::iter(logs.into_iter().map(|log| on_log.call(LogLevel::Warn, log)))
    .buffer_unordered(MAX_IN_FLIGHT_WARNINGS)
    .try_collect::<Vec<()>>()
    .await?;

  Ok(())
}
