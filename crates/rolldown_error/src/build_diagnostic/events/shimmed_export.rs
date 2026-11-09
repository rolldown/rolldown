use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

use super::BuildEvent;

#[derive(Debug)]
pub struct ShimmedExport {
  pub exporter: String,
  pub binding: String,
}

impl BuildEvent for ShimmedExport {
  fn kind(&self) -> EventKind {
    EventKind::ShimmedExport
  }

  fn exporter(&self) -> Option<String> {
    Some(self.exporter.clone())
  }

  fn binding(&self) -> Option<String> {
    Some(self.binding.clone())
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    format!(
      "Missing export \"{}\" has been shimmed in module \"{}\".",
      self.binding,
      opts.stabilize_path(&self.exporter)
    )
  }
}
