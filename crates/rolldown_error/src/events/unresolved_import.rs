use super::BuildEvent;
use crate::PathExt;
use std::path::PathBuf;

#[derive(Debug)]
pub struct UnresolvedImport {
  pub(crate) specifier: String,
  pub(crate) importer: PathBuf,
}

impl BuildEvent for UnresolvedImport {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnresolvedImport
  }
  fn code(&self) -> &'static str {
    "UNRESOLVED_IMPORT"
  }

  fn message(&self) -> String {
    format!("Could not resolve {} from {}.", self.specifier, self.importer.relative_display())
  }
}
