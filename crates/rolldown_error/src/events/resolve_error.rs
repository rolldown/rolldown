use crate::types::diagnostic_options::DiagnosticOptions;
use arcstr::ArcStr;

use super::{BuildEvent, DiagnosableArcstr};

#[derive(Debug)]
pub struct DiagnosableResolveError {
  pub source: ArcStr,
  pub importer_id: ArcStr,
  pub importee: DiagnosableArcstr,
  pub reason: String,
  pub title: Option<&'static str>,
}

impl BuildEvent for DiagnosableResolveError {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::ResolveError(self.title)
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let importee = match &self.importee {
      DiagnosableArcstr::String(str) => str.as_str(),
      DiagnosableArcstr::Span(span) => &self.source.as_str()[*span],
    };
    format!("Could not resolve {} in {}", importee, opts.stabilize_path(self.importer_id.as_str()))
  }

  fn on_diagnostic(
    &self,
    diagnostic: &mut crate::diagnostic::Diagnostic,
    opts: &DiagnosticOptions,
  ) {
    let stable_id = opts.stabilize_path(self.importer_id.as_str());
    let importer_file = diagnostic.add_file(stable_id, self.source.clone());

    match self.importee {
      DiagnosableArcstr::Span(span) if !span.is_unspanned() => {
        diagnostic.add_label(&importer_file, span.start..span.end, self.reason.clone());
      }
      _ => {}
    };
    diagnostic.title = self.message(opts);
  }
}
