use std::sync::Arc;

use super::BuildEvent;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions, EventKind};

#[derive(Debug)]
pub struct AmbiguousExternalNamespace {
  pub importer: String,
  pub importee: Vec<String>,
  pub importer_source: Arc<str>,
  pub importer_filename: String,
  pub imported_specifier: String,
  pub imported_specifier_span: Span,
}

impl BuildEvent for AmbiguousExternalNamespace {
  fn kind(&self) -> EventKind {
    EventKind::AmbiguousExternalNamespace
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      r#""{}" re-exports "{}" from one of the modules {} (will be ignored)."#,
      self.importer,
      self.imported_specifier,
      self.importee.iter().map(|v| format!(r#""{v}""#)).collect::<Vec<_>>().join(" and ")
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id =
      diagnostic.add_file(self.importer_filename.clone(), Arc::clone(&self.importer_source));

    diagnostic.title = "Warning: Found ambiguous export.".to_string();

    diagnostic.add_label(
      &file_id,
      self.imported_specifier_span.start..self.imported_specifier_span.end,
      self.message(opts),
    );
  }
}
