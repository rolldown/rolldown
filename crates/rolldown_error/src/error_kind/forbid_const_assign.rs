use std::{path::Path, sync::Arc};

use ariadne::Label;
use oxc::span::Span;

use crate::{diagnostic::DiagnosticBuilder, PathExt};

use super::BuildErrorLike;

#[derive(Debug)]
pub struct ForbidConstAssign {
  pub filename: String,
  pub source: Arc<str>,
  pub name: String,
  pub reference_span: Span,
  pub re_assign_span: Span,
}

impl BuildErrorLike for ForbidConstAssign {
  //
  fn code(&self) -> &'static str {
    "FORBID_CONST_ASSIGN"
  }

  fn message(&self) -> String {
    format!("Unexpected re-assignment of const variable `{0}` at {1}", self.name, self.filename)
  }

  fn diagnostic_builder(&self) -> crate::diagnostic::DiagnosticBuilder {
    let filename = Path::new(&self.filename).relative_display();
    DiagnosticBuilder {
      code: Some(self.code()),
      summary: Some(format!("Unexpected re-assignment of const variable `{0}`", self.name)),
      files: Some(vec![(filename.clone(), self.source.to_string())]),
      labels: Some(vec![
        Label::new((
          filename.clone(),
          (self.re_assign_span.start as usize..self.re_assign_span.end as usize),
        ))
        .with_message(format!("`{0}` is re-assigned here", self.name)),
        Label::new((
          filename,
          (self.reference_span.start as usize..self.reference_span.end as usize),
        ))
        .with_message(format!("`{0}` is declared here as const", self.name)),
      ]),
      ..Default::default()
    }
  }
}
