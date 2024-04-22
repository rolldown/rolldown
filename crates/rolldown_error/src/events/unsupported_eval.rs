use std::{path::Path, sync::Arc};

use oxc::span::Span;

use crate::{diagnostic::Diagnostic, PathExt};

use super::BuildEvent;

#[derive(Debug)]
pub struct UnsupportedEval {
  pub filename: String,
  pub source: Arc<str>,
  pub eval_span: Span,
}

impl BuildEvent for UnsupportedEval {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::Eval
  }

  fn code(&self) -> &'static str {
    "UNSUPPORTED_EVAL"
  }

  fn message(&self) -> String {
    format!("Unsupported eval at {}", self.filename)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic) {
    let filename = Path::new(&self.filename).relative_display();

    diagnostic.title = "Rolldown does not support `eval` function currently.".to_string();

    let file_id = diagnostic.add_file(filename, Arc::clone(&self.source));

    diagnostic.add_label(
      &file_id,
      self.eval_span.start..self.eval_span.end,
      "Used `eval` function here.".to_string(),
    );
  }
}
