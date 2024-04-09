use crate::{event_kind::EventKind, PathExt};
use std::path::PathBuf;

use super::BuildEvent;

#[derive(Debug)]
pub struct ExternalEntry {
  pub(crate) id: PathBuf,
}

impl BuildEvent for ExternalEntry {
  fn kind(&self) -> crate::event_kind::EventKind {
    EventKind::UnresolvedEntry
  }

  fn code(&self) -> &'static str {
    "UNRESOLVED_ENTRY"
  }

  fn message(&self) -> String {
    format!("Entry module {} cannot be external.", self.id.relative_display())
  }
}
