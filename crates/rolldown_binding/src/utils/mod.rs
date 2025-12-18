mod normalize_binding_transform_options;

pub mod create_bundler_from_binding_options;
pub mod create_bundler_options_from_binding_options;
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
      on_log
        .call(
          LogLevel::Warn,
          rolldown::Log {
            id: warning.id(),
            exporter: warning.exporter(),
            code: Some(warning.kind().to_string()),
            message: warning
              .to_diagnostic_with(&DiagnosticOptions { cwd: options.cwd.clone() })
              .to_color_string(),
            plugin: None,
          },
        )
        .await?;
    }
  }
  Ok(())
}
