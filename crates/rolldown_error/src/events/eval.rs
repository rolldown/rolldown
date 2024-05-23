use std::sync::Arc;

use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct Eval {
  pub filename: String,
  pub source: Arc<str>,
  pub eval_span: Span,
}

impl BuildEvent for Eval {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::Eval
  }

  fn code(&self) -> &'static str {
    "EVAL"
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Use of eval in '{}' is strongly discouraged as it poses security risks and may cause issues with minification.", self.filename)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);

    diagnostic.title = "Use of eval is strongly discouraged as it poses security risks and may cause issues with minification.".to_string();

    let file_id = diagnostic.add_file(filename, Arc::clone(&self.source));

    diagnostic.add_label(
      &file_id,
      self.eval_span.start..self.eval_span.end,
      "Used `eval` function at here.".to_string(),
    );
  }
}
