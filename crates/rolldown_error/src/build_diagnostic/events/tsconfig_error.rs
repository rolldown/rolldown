use oxc_resolver::ResolveError;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  utils::resolve_error_to_message,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct TsConfigError {
  pub file_path: String,
  pub reason: ResolveError,
}

impl TsConfigError {
  /// Generates the error message with stabilized paths for the tsconfig error.
  fn generate_message(&self, opts: &DiagnosticOptions) -> String {
    let reason_msg = match &self.reason {
      ResolveError::Json(json_error) => {
        format!("JSON parse error in '{}'", opts.stabilize_path(&json_error.path))
      }
      _ => resolve_error_to_message(&self.reason),
    };
    format!(
      "Failed to load tsconfig for '{}': {}",
      opts.stabilize_path(&self.file_path),
      reason_msg
    )
  }
}

impl BuildEvent for TsConfigError {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::TsConfigError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    self.generate_message(opts)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title = self.message(opts);
  }
}
