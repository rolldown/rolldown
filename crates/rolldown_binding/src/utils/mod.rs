mod normalize_binding_transform_options;

pub mod collapse_sourcemaps;
pub mod create_bundler_config_from_binding_options;
pub mod minify_options_conversion;
pub mod napi_error;
pub mod normalize_binding_options;

use std::any::Any;

use napi_derive::napi;
use rolldown::{LogLevel, NormalizedBundlerOptions};
use rolldown_error::{BuildDiagnostic, DiagnosticOptions, filter_out_disabled_diagnostics};
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
  if options.log_level == Some(LogLevel::Silent) {
    return Ok(());
  }
  if let Some(on_log) = options.on_log.as_ref() {
    for warning in filter_out_disabled_diagnostics(warnings, &options.checks) {
      let diag = warning.to_diagnostic_with(&DiagnosticOptions { cwd: options.cwd.clone() });
      let code = warning.kind().to_string();

      // Extract location information from the diagnostic if available
      // Only include loc/pos for warning types that report specific source locations.
      // Note: Line numbers, columns, and byte positions are cast to u32.
      // This is safe for practical use cases as files with >4 billion lines or bytes are extremely rare.
      // Use warning.id() for the file path since the diagnostic may only store the filename.
      #[expect(
        clippy::cast_possible_truncation,
        reason = "line/column/position values are unlikely to exceed u32::MAX in practical use"
      )]
      let (loc, pos) = if let Some((_file, line, column, position)) = diag.get_primary_location() {
        (
          Some(rolldown::LogLocation {
            line: line as u32,
            column: column as u32,
            file: warning.id(),
          }),
          Some(position as u32),
        )
      } else {
        (None, None)
      };

      on_log
        .call(
          LogLevel::Warn,
          rolldown::Log {
            id: warning.id(),
            exporter: warning.exporter(),
            code: Some(code),
            message: diag.to_color_string(),
            plugin: None,
            loc,
            pos,
            ids: warning.ids(),
          },
        )
        .await?;
    }
  }
  Ok(())
}
