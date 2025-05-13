use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct ForbidConstAssign {
  pub filename: String,
  pub source: ArcStr,
  pub name: String,
  pub reference_span: Span,
  pub re_assign_span: Span,
}

impl BuildEvent for ForbidConstAssign {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::IllegalReassignmentError
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Unexpected re-assignment of const variable `{0}` at {1}", self.name, self.filename)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);
    diagnostic.title = format!("Unexpected re-assignment of const variable `{0}`", self.name);

    let file_id = diagnostic.add_file(filename, self.source.clone());
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
