use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct UntranspiledSyntax {
  pub filename: String,
  pub syntax_kind: &'static str,
}

impl BuildEvent for UntranspiledSyntax {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::UntranspiledSyntaxError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let filename = opts.stabilize_path(&self.filename);
    format!(
      "{} syntax should be transpiled before bundling. Found untranspiled {} in {filename:?}",
      self.syntax_kind, self.syntax_kind
    )
  }

  fn on_diagnostic(&self, _diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {}
}
