use arcstr::ArcStr;
use oxc::span::Span;

use crate::{event_kind::EventKind, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct MissingExport {
  pub importer: String,
  pub stable_importer: String,
  pub stable_importee: String,
  pub importer_source: ArcStr,
  pub imported_specifier: String,
  pub imported_specifier_span: Span,
  pub note: Option<String>,
}

impl BuildEvent for MissingExport {
  fn kind(&self) -> crate::event_kind::EventKind {
    EventKind::MissingExportError
  }

  fn id(&self) -> Option<String> {
    Some(self.importer.clone())
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
    let file_id = diagnostic.add_file(&self.stable_importer, &self.importer_source);

    diagnostic.title =
      format!(r#""{}" is not exported by "{}"."#, self.imported_specifier, &self.stable_importee);

    if let Some(note) = &self.note {
      diagnostic.add_note(note.clone());
    }

    diagnostic.add_label(
      &file_id,
      self.imported_specifier_span.start..self.imported_specifier_span.end,
      String::from("Missing export"),
    );
  }
}
