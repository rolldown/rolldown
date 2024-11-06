use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct UnsupportedFeature {
  pub(crate) source: ArcStr,
  pub(crate) filename: ArcStr,
  pub(crate) span: Span,
  pub(crate) error_message: String,
}

impl BuildEvent for UnsupportedFeature {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnsupportedFeature
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {
    diagnostic.title.clone_from(&self.error_message);

    let file_id = diagnostic.add_file(self.filename.clone(), self.source.clone());
    diagnostic.add_label(&file_id, self.span.start..self.span.end, "".to_string());
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.error_message.clone()
  }
}
