use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
#[error("Entry module \"{:?}\" cannot be external.", id)]
pub struct ExternalEntry {
  pub(crate) id: PathBuf,
}
