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

impl BuildEvent for TsConfigError {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::TsConfigError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "Failed to load tsconfig for '{}': {}",
      opts.stabilize_path(&self.file_path),
      resolve_error_to_message(&self.reason)
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title = self.message(opts);
  }
}
