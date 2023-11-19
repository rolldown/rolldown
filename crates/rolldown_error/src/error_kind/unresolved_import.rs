use super::BuildErrorLike;
use crate::{PathExt, StaticStr};
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

impl BuildErrorLike for UnresolvedImport {
  fn code(&self) -> &'static str {
    "UNRESOLVED_IMPORT"
  }

  fn message(&self) -> String {
    format!("Could not resolve {} from {}.", self.specifier, self.importer.relative_display())
  }
}
