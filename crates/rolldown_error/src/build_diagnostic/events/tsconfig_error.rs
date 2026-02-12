use oxc_resolver::ResolveError;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  utils::resolve_error_to_message,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct TsConfigError {
  pub file_paths: Vec<String>,
  pub reason: ResolveError,
}

impl TsConfigError {
  /// Merge file paths from another TsConfigError into this one
  pub fn merge(&mut self, file_paths: Vec<String>) {
    self.file_paths.extend(file_paths);
  }
}

impl BuildEvent for TsConfigError {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::TsConfigError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let mut stabilized_paths =
      self.file_paths.iter().map(|p| opts.stabilize_path(p)).collect::<Vec<_>>();
    stabilized_paths.sort();
    let file_list =
      stabilized_paths.into_iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(", ");
    format!(
      "Failed to load tsconfig for {}: {}",
      file_list,
      resolve_error_to_message(&self.reason, opts)
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title = self.message(opts);
  }
}
