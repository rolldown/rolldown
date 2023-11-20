use crate::PathExt;
use std::path::PathBuf;

use super::BuildErrorLike;

#[derive(Debug)]
pub struct ExternalEntry {
  pub(crate) id: PathBuf,
}

impl BuildErrorLike for ExternalEntry {
  fn code(&self) -> &'static str {
    "UNRESOLVED_ENTRY"
  }

  fn message(&self) -> String {
    format!("Entry module {} cannot be external.", self.id.relative_display())
  }
}
