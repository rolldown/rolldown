use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

#[derive(Debug)]
pub struct CircularReexport {
  pub importer_id: String,
  pub imported_specifier: String,
}

impl BuildEvent for CircularReexport {
  fn kind(&self) -> EventKind {
    EventKind::CircularReexportError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!(
      "'{}' cannot be exported from '{}' as it is a reexport that references itself.",
      self.imported_specifier, self.importer_id
    )
  }

  fn id(&self) -> Option<String> {
    Some(self.importer_id.clone())
  }
}
