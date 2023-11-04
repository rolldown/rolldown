use crate::PathExt;
use std::path::PathBuf;
use thiserror::Error;
#[derive(Error, Debug)]
#[error("Cannot resolve entry module {:?}", unresolved_id.relative_display())]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
}
