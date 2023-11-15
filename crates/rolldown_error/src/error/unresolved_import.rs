use super::StaticStr;
use crate::PathExt;
use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[diagnostic(code = "UNRESOLVED_IMPORT")]
#[error("Could not resolve {} from {}.", specifier, importer.relative_display())]
pub struct UnresolvedImport {
  pub(crate) specifier: StaticStr,
  pub(crate) importer: PathBuf,
}
