use crate::{EventKind, types::diagnostic_options::DiagnosticOptions};
use arcstr::ArcStr;
use derive_more::Debug;

use super::{BuildEvent, DiagnosableArcstr};

#[derive(Debug)]
pub struct DiagnosableResolveError {
  pub source: ArcStr,
  pub importer_id: ArcStr,
  pub importee: DiagnosableArcstr,
  pub reason: String,
  pub help: Option<String>,
  #[debug(skip)]
  pub diagnostic_kind: EventKind,
}

impl DiagnosableResolveError {
  fn importee_str(&self) -> &str {
    let s = match &self.importee {
      DiagnosableArcstr::String(str) => str.as_str(),
      DiagnosableArcstr::Span(span) => &self.source.as_str()[*span],
    };
    &s[1..s.len() - 1]
  }
}

impl BuildEvent for DiagnosableResolveError {
  fn kind(&self) -> crate::event_kind::EventKind {
    self.diagnostic_kind
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "Could not resolve '{}' in {}",
      self.importee_str(),
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

    match self.importee {
      DiagnosableArcstr::Span(span) if !span.is_unspanned() => {
        diagnostic.add_label(&importer_file, span.start..span.end, self.reason.clone());
      }
      _ => {}
    };
    diagnostic.title = self.message(opts);
    diagnostic.help.clone_from(&self.help);
  }

  fn id(&self) -> Option<String> {
    Some(self.importer_id.to_string())
  }

  fn exporter(&self) -> Option<String> {
    Some(self.importee_str().to_string())
  }
}
