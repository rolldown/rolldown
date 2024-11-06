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

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title.clone_from(&self.error_message);

    let file_id =
      diagnostic.add_file(opts.stabilize_path(self.filename.as_str()), self.source.clone());
    diagnostic.add_label(&file_id, self.span.start..self.span.end, String::new());
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.error_message.clone()
  }
}
