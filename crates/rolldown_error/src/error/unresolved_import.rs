use std::path::PathBuf;

use thiserror::Error;

use super::StaticStr;

#[derive(Error, Debug)]
#[error(r#"Could not resolve "{}" from "{:?}"#, specifier, importer)]
pub struct UnresolvedImport {
  pub(crate) specifier: StaticStr,
  pub(crate) importer: PathBuf,
}
