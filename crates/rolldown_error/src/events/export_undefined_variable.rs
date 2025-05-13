use arcstr::ArcStr;
use oxc::span::Span;

use crate::{diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions};

use super::BuildEvent;

#[derive(Debug)]
pub struct ExportUndefinedVariable {
  pub filename: String,
  pub source: ArcStr,
  pub span: Span,
  pub name: ArcStr,
}

impl BuildEvent for ExportUndefinedVariable {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::ExportUndefinedVariableError
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("`{}` is not declared in this file", self.name)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&self.filename);

    let file_id = diagnostic.add_file(filename, self.source.clone());

    diagnostic.add_label(&file_id, self.span.start..self.span.end, String::new());
  }
}
