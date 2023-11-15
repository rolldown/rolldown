use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

pub mod external_entry;
// pub mod impl_to_diagnostic;
pub mod unresolved_entry;
pub mod unresolved_import;

use miette::Diagnostic;
use thiserror::Error;

use self::{
  external_entry::ExternalEntry, unresolved_entry::UnresolvedEntry,
  unresolved_import::UnresolvedImport,
};

type StaticStr = Cow<'static, str>;

#[derive(Error, Debug, Diagnostic)]
pub enum BuildError {
  #[diagnostic(transparent)]
  #[error(transparent)]
  UnresolvedEntry(Box<UnresolvedEntry>),

  #[diagnostic(transparent)]
  #[error(transparent)]
  ExternalEntry(Box<ExternalEntry>),

  #[diagnostic(transparent)]
  #[error(transparent)]
  UnresolvedImport(Box<UnresolvedImport>),

  #[diagnostic(code = "IO_ERROR")]
  #[error(transparent)]
  Io(Box<std::io::Error>),

  // TODO: probably should remove this error
  #[diagnostic(code = "NAPI_ERROR")]
  #[error("Napi error: {status}: {reason}")]
  Napi { status: String, reason: String },
}

impl BuildError {
  // --- Aligned with rollup
  pub fn entry_cannot_be_external(unresolved_id: impl AsRef<Path>) -> Self {
    Self::ExternalEntry(ExternalEntry { id: unresolved_id.as_ref().to_path_buf() }.into())
  }

  pub fn unresolved_entry(unresolved_id: impl AsRef<Path>) -> Self {
    Self::UnresolvedEntry(
      UnresolvedEntry { unresolved_id: unresolved_id.as_ref().to_path_buf() }.into(),
    )
  }

  pub fn unresolved_import(specifier: impl Into<StaticStr>, importer: impl Into<PathBuf>) -> Self {
    Self::UnresolvedImport(
      UnresolvedImport { specifier: specifier.into(), importer: importer.into() }.into(),
    )
  }

  // --- rolldown specific
  pub fn napi_error(status: String, reason: String) -> Self {
    Self::Napi { status, reason }
  }
}

impl From<std::io::Error> for BuildError {
  fn from(e: std::io::Error) -> Self {
    Self::Io(Box::new(e))
  }
}
