use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Entry module \"{:?}\" cannot be external.", id)]
pub struct ExternalEntry {
  pub(crate) id: PathBuf,
}
