use std::sync::Arc;

use crate::types::diagnostic_options::DiagnosticOptions;

use super::BuildEvent;

#[derive(Debug)]
pub struct SourcemapBroken {
  pub plugin_name: Option<Arc<str>>,
}

impl BuildEvent for SourcemapBroken {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::SourcemapBroken
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Sourcemap is likely to be incorrect: a plugin ({}) was used to transform files, but didn't generate a sourcemap for the transformation. Consult the plugin documentation for help", self.plugin_name.clone().unwrap_or("unknown name".into()))
  }
}
