use super::BuildErrorLike;
use crate::{PathExt, StaticStr};
use std::path::PathBuf;

#[derive(Debug)]
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
