use arcstr::ArcStr;
use oxc::span::{CompactStr, Span};

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct JsonParse {
  pub filename: CompactStr,
  pub source: ArcStr,
  pub span: Span,
  pub message: CompactStr,
}

impl BuildEvent for JsonParse {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::JsonParseError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    self.message.to_string()
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    if !self.span.is_unspanned() {
      let filename = opts.stabilize_path(self.filename.as_str());
      let file_id = diagnostic.add_file(filename, self.source.clone());
      diagnostic.add_label(&file_id, self.span.start..self.span.end, self.message.to_string());
    }
  }
}
