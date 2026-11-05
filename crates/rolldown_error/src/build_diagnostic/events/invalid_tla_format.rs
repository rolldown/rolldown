use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct InvalidTlaFormat {
  pub(crate) filename: ArcStr,
  pub(crate) source: ArcStr,
  pub(crate) span: Span,
  pub(crate) format: ArcStr,
}

impl BuildEvent for InvalidTlaFormat {
  fn kind(&self) -> EventKind {
    EventKind::InvalidTlaFormatError
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Module format \"{}\" does not support top-level await. Use the \"es\" output format instead.",
      self.format
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id =
      diagnostic.add_file(opts.stabilize_path(self.filename.as_str()), self.source.clone());
    diagnostic.add_label(&file_id, self.span.start..self.span.end, String::new());
  }
}
