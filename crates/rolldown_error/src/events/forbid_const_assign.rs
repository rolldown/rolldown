use std::{path::Path, sync::Arc};

use oxc::span::Span;

use crate::{diagnostic::Diagnostic, PathExt};

use super::BuildEvent;

#[derive(Debug)]
pub struct ForbidConstAssign {
  pub filename: String,
  pub source: Arc<str>,
  pub name: String,
  pub reference_span: Span,
  pub re_assign_span: Span,
}

impl BuildEvent for ForbidConstAssign {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::IllegalReassignment
  }
  fn code(&self) -> &'static str {
    "FORBID_CONST_ASSIGN"
  }

  fn message(&self) -> String {
    format!("Unexpected re-assignment of const variable `{0}` at {1}", self.name, self.filename)
  }
  fn on_diagnostic(&self, diagnostic: &mut Diagnostic) {
    let filename = Path::new(&self.filename).relative_display();
    diagnostic.title = format!("Unexpected re-assignment of const variable `{0}`", self.name);

    let file_id = diagnostic.add_file(filename, Arc::clone(&self.source));
    diagnostic
      .add_label(
        &file_id,
        self.re_assign_span.start..self.re_assign_span.end,
        format!("`{0}` is re-assigned here", self.name),
      )
      .add_label(
        &file_id,
        self.reference_span.start..self.reference_span.end,
        format!("`{0}` is declared here as const", self.name),
      );
  }
}
