use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct SourceMapBroken {
  pub plugin_name: String,
  pub sourcemap_type: String,
  pub hook_name: String,
}

impl BuildEvent for SourceMapBroken {
  fn kind(&self) -> EventKind {
    EventKind::SourceMapBroken
  }
  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Sourcemap is likely to be incorrect: options.sourcemap  is {}, a plugin {} for  {} didn't generate a sourcemap.",
      self.sourcemap_type, self.plugin_name, self.hook_name
    )
  }
}
