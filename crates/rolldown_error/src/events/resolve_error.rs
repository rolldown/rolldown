use crate::types::diagnostic_options::DiagnosticOptions;
use arcstr::ArcStr;
use oxc::span::Span;

use super::BuildEvent;

#[derive(Debug)]
pub struct DiagnosableResolveError {
  pub source: ArcStr,
  pub importer_id: ArcStr,
  pub importee_span: Span,
  pub reason: String,
}

impl BuildEvent for DiagnosableResolveError {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::DiagnosableResolveError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let start = self.importee_span.start as usize;
    let end = self.importee_span.end as usize;
    format!(
      "Could not resolve {} in {}",
      &self.source[start..end],
      opts.stabilize_path(self.importer_id.as_str())
    )
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::diagnostic::Diagnostic,
    opts: &DiagnosticOptions,
  ) {
    let stable_id = opts.stabilize_path(self.importer_id.as_str());
    let importer_file = diagnostic.add_file(stable_id, self.source.clone());

    diagnostic.add_label(
      &importer_file,
      self.importee_span.start..self.importee_span.end,
      self.reason.clone(),
    );
    diagnostic.title = self.message(opts);
  }
}
