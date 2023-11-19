use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;

use crate::error_kind::{
  external_entry::ExternalEntry, unresolved_entry::UnresolvedEntry,
  unresolved_import::UnresolvedImport, ErrorKind,
};

type StaticStr = Cow<'static, str>;

#[derive(Error, Debug, Diagnostic)]
#[error(transparent)]
#[diagnostic(transparent)]
pub struct BuildError(ErrorKind);

impl BuildError {
  pub fn new_with_kind(kind: ErrorKind) -> Self {
    Self(kind)
  }

  // --- Aligned with rollup
  pub fn entry_cannot_be_external(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_with_kind(ErrorKind::ExternalEntry(
      ExternalEntry { id: unresolved_id.as_ref().to_path_buf() }.into(),
    ))
  }

  pub fn unresolved_entry(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_with_kind(ErrorKind::UnresolvedEntry(
      UnresolvedEntry { unresolved_id: unresolved_id.as_ref().to_path_buf() }.into(),
    ))
  }

  pub fn unresolved_import(specifier: impl Into<StaticStr>, importer: impl Into<PathBuf>) -> Self {
    Self::new_with_kind(ErrorKind::UnresolvedImport(
      UnresolvedImport { specifier: specifier.into(), importer: importer.into() }.into(),
    ))
  }

  // --- rolldown specific
  pub fn napi_error(status: String, reason: String) -> Self {
    Self::new_with_kind(ErrorKind::Napi { status, reason })
  }
}

impl From<std::io::Error> for BuildError {
  fn from(e: std::io::Error) -> Self {
    Self::new_with_kind(ErrorKind::Io(Box::new(e)))
  }
}
