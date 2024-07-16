use super::BuildEvent;
use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions, EventKind};

#[derive(Debug)]
pub struct AmbiguousExternalNamespace {
  pub importer: String,
  pub importee: Vec<String>,
  pub importer_source: ArcStr,
  pub importer_filename: String,
  pub imported_specifier: String,
  pub imported_specifier_span: Span,
}

impl BuildEvent for AmbiguousExternalNamespace {
  fn kind(&self) -> EventKind {
    EventKind::AmbiguousExternalNamespace
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    let mut importee = self.importee.iter().map(|v| format!(r#""{v}""#));

    let last = importee.next_back().unwrap();

    format!(
      r#""{}" re-exports "{}" from one of the modules {} and {} (will be ignored)."#,
      self.importer,
      self.imported_specifier,
      importee.collect::<Vec<_>>().join(", "),
      last
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id = diagnostic.add_file(self.importer_filename.clone(), self.importer_source.clone());

    diagnostic.title = "Found ambiguous export.".to_string();

    diagnostic.add_label(
      &file_id,
      self.imported_specifier_span.start..self.imported_specifier_span.end,
      self.message(opts),
    );
  }
}
