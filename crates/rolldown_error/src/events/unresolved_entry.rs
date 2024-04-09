use crate::PathExt;
use std::path::PathBuf;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
}

impl BuildEvent for UnresolvedEntry {
  fn kind(&self) -> crate::event_kind::EventKind {
    crate::event_kind::EventKind::UnresolvedEntry
  }
  fn code(&self) -> &'static str {
    "UNRESOLVED_ENTRY"
  }

  fn message(&self) -> String {
    format!("Cannot resolve entry module {}.", self.unresolved_id.relative_display())
  }
}
