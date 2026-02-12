use arcstr::ArcStr;
use oxc::span::Span;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct CannotCallNamespace {
  pub filename: ArcStr,
  pub source: ArcStr,
  pub span: Span,
  pub name: ArcStr,
  pub declaration_span: Span,
}

impl BuildEvent for CannotCallNamespace {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::CannotCallNamespace
  }

  fn id(&self) -> Option<String> {
    Some(self.filename.to_string())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Cannot call a namespace (\"{}\").", self.name)
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let filename = opts.stabilize_path(&*self.filename);

    let file_id = diagnostic.add_file(filename, self.source.clone());
    diagnostic
      .add_label(
        &file_id,
        self.span.start..self.span.end,
        format!("This will cause an error at runtime because \"{}\" is a module namespace object and not a function. Consider changing \"{}\" to a default import instead.", self.name, self.name)
      )
      .add_label(
        &file_id,
        self.declaration_span.start..self.declaration_span.end,
        format!("\"{}\" is imported as a namespace here", self.name)
      );
  }
}
