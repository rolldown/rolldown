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
    crate::types::event_kind::EventKind::UnhandleableError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let filename = opts.stabilize_path(&self.filename);
    format!(
      "Something went wrong inside rolldown while processing {filename:?}: found untranspiled {} syntax. Please report this problem at https://github.com/rolldown/rolldown/issues",
      self.syntax_kind
    )
  }

  fn on_diagnostic(&self, _diagnostic: &mut Diagnostic, _opts: &DiagnosticOptions) {}
}
