use crate::PathExt;
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

use super::BuildErrorLike;

#[derive(Error, Debug, Diagnostic)]
#[diagnostic(code = "UNRESOLVED_ENTRY")]
#[error("Cannot resolve entry module {}.", unresolved_id.relative_display())]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
}

impl BuildErrorLike for UnresolvedEntry {
  fn code(&self) -> &'static str {
    "UNRESOLVED_ENTRY"
  }

  fn message(&self) -> String {
    format!("Cannot resolve entry module {}.", self.unresolved_id.relative_display())
  }
}
