use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;
use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
};

#[derive(Debug)]
pub struct DeprecatedImportAssertion {
  pub module_id: String,
  pub source: ArcStr,
  pub span: Span,
}

impl BuildEvent for DeprecatedImportAssertion {
  fn kind(&self) -> EventKind {
    EventKind::DeprecatedImportAssertion
  }

  fn id(&self) -> Option<String> {
    Some(self.module_id.clone())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "The legacy `assert` keyword in \"{}\" is deprecated and is treated as `with`. Use `with` instead.",
      opts.stabilize_path(&self.module_id)
    )
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    let file_id = diagnostic.add_file(opts.stabilize_path(&self.module_id), self.source.clone());
    diagnostic.add_label(
      &file_id,
      self.span.start..self.span.end,
      "replace `assert` with `with`".to_string(),
    );
  }
}
