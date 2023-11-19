use std::{path::Path, sync::Arc};

use ariadne::Label;
use oxc::span::Span;

use crate::{diagnostic::DiagnosticBuilder, PathExt};

use super::BuildErrorLike;

#[derive(Debug)]
pub struct UnsupportedEval {
  pub filename: String,
  pub source: Arc<str>,
  pub eval_span: Span,
}

impl BuildErrorLike for UnsupportedEval {
  //
  fn code(&self) -> &'static str {
    "UNSUPPORTED_EVAL"
  }

  fn message(&self) -> String {
    format!("Unsupported eval at {}", self.filename)
  }

  fn diagnostic_builder(&self) -> crate::diagnostic::DiagnosticBuilder {
    let filename = Path::new(&self.filename).relative_display();
    DiagnosticBuilder {
      code: Some(self.code()),
      summary: Some("Rolldown does not support `eval` function currently.".to_string()),
      files: Some(vec![(filename.clone(), self.source.to_string())]),
      labels: Some(vec![Label::new((
        filename,
        (self.eval_span.start as usize..self.eval_span.end as usize),
      ))
      .with_message("Used `eval` function here.")]),
      ..Default::default()
    }
  }
}
