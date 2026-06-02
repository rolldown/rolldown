use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct SourcemapBroken {
  pub plugin_name: String,
  /// The id of the module whose `transform` broke the sourcemap chain. `None`
  /// for `renderChunk`, which operates on a chunk rather than a module.
  pub id: Option<String>,
}

impl BuildEvent for SourcemapBroken {
  fn kind(&self) -> EventKind {
    EventKind::SourcemapBroken
  }

  fn id(&self) -> Option<String> {
    self.id.clone()
  }

  fn plugin(&self) -> Option<String> {
    Some(self.plugin_name.clone())
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "Sourcemap is likely to be incorrect: a plugin ({}) was used to transform files, but didn't generate a sourcemap for the transformation. Consult the plugin documentation for help: https://rolldown.rs/guide/troubleshooting#warning-sourcemap-is-likely-to-be-incorrect",
      self.plugin_name
    )
  }
}
