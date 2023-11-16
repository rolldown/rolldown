use crate::PathExt;
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[diagnostic(code = "UNRESOLVED_ENTRY")]
#[error("Entry module {} cannot be external.", id.relative_display())]
pub struct ExternalEntry {
  pub(crate) id: PathBuf,
}
