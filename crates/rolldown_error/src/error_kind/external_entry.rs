use crate::PathExt;
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

use super::BuildErrorLike;

#[derive(Error, Debug, Diagnostic)]
#[diagnostic(code = "UNRESOLVED_ENTRY")]
#[error("Entry module {} cannot be external.", id.relative_display())]
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
