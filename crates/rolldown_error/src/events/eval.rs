use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct Eval {
  pub span: Span,
  pub source: ArcStr,
  pub filename: String,
}

impl BuildEvent for Eval {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::Eval
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Use of `eval` function in '{}' is strongly discouraged as it poses security risks and may cause issues with minification.",
      self.filename
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);
    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.title = String::from(
      "Use of `eval` function is strongly discouraged as it poses security risks and may cause issues with minification.",
    );

    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      String::from("Use of `eval` function here."),
    );
  }
}
