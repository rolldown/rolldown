use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use super::StaticStr;

#[derive(Error, Debug, Diagnostic)]
#[error(r#"Could not resolve "{}" from "{:?}"#, specifier, importer)]
pub struct UnresolvedImport {
  pub(crate) specifier: StaticStr,
  pub(crate) importer: PathBuf,
}
