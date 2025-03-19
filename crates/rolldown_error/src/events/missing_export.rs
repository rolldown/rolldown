use arcstr::ArcStr;
use oxc::span::Span;

use crate::{event_kind::EventKind, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct MissingExport {
  pub stable_importer: String,
  pub stable_importee: String,
  pub importer_source: ArcStr,
  pub imported_specifier: String,
  pub imported_specifier_span: Span,
}

impl BuildEvent for MissingExport {
  fn kind(&self) -> crate::event_kind::EventKind {
    EventKind::MissingExportError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      r#""{}" is not exported by "{}", imported by "{}"."#,
      self.imported_specifier, &self.stable_importee, &self.stable_importer
    )
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::diagnostic::Diagnostic,
    _opts: &DiagnosticOptions,
  ) {
    let importer_file =
      diagnostic.add_file(self.stable_importer.clone(), self.importer_source.clone());

    diagnostic.title =
      format!(r#""{}" is not exported by "{}"."#, self.imported_specifier, &self.stable_importee);

    diagnostic.add_label(
      &importer_file,
      self.imported_specifier_span.start..self.imported_specifier_span.end,
      "Missing export".to_string(),
    );
  }
}
