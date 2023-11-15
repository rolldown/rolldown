use crate::PathExt;
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[diagnostic(code = "UNRESOLVED_ENTRY")]
#[error("Cannot resolve entry module {}.", unresolved_id.relative_display())]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
}
