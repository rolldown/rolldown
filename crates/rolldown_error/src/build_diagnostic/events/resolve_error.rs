use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};
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
  pub import_chain: Option<Vec<String>>,
}

impl DiagnosableResolveError {
  fn importee_str(&self) -> &str {
    match &self.importee {
      DiagnosableArcstr::String(str) => str.as_str(),
      DiagnosableArcstr::Span(span) => {
        let s = &self.source.as_str()[*span];
        &s[1..s.len() - 1]
      }
    }
  }
}

impl BuildEvent for DiagnosableResolveError {
  fn kind(&self) -> crate::types::event_kind::EventKind {
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
    diagnostic: &mut crate::build_diagnostic::diagnostic::Diagnostic,
    opts: &DiagnosticOptions,
  ) {
    let stable_id = opts.stabilize_path(self.importer_id.as_str());
    let importer_file = diagnostic.add_file(stable_id, self.source.clone());

    match self.importee {
      DiagnosableArcstr::Span(span) if !span.is_unspanned() => {
        diagnostic.add_label(&importer_file, span.start..span.end, self.reason.clone());
      }
      _ => {}
    }
    diagnostic.title = self.message(opts);
    
    // Build the help message with import chain if available
    let mut help_message = self.help.clone();
    if let Some(chain) = &self.import_chain {
      if !chain.is_empty() {
        let chain_text = format!(
          "'{}' is imported by the following path:\n{}",
          opts.stabilize_path(self.importer_id.as_str()),
          chain.iter().map(|p| format!("  - {}", opts.stabilize_path(p))).collect::<Vec<_>>().join("\n")
        );
        help_message = Some(match help_message {
          Some(existing) => format!("{}\n\n{}", existing, chain_text),
          None => chain_text,
        });
      }
    }
    diagnostic.help = help_message;
  }

  fn id(&self) -> Option<String> {
    Some(self.importer_id.to_string())
  }

  fn exporter(&self) -> Option<String> {
    Some(self.importee_str().to_string())
  }
}
