use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

pub mod external_entry;
pub mod impl_to_diagnostic;
pub mod unresolved_entry;
pub mod unresolved_import;

use thiserror::Error;

use crate::error_code;

use self::{
  external_entry::ExternalEntry, unresolved_entry::UnresolvedEntry,
  unresolved_import::UnresolvedImport,
};

type StaticStr = Cow<'static, str>;

#[derive(Error, Debug)]
pub enum BuildError {
  #[error(transparent)]
  UnresolvedEntry(Box<UnresolvedEntry>),
  #[error(transparent)]
  ExternalEntry(Box<ExternalEntry>),
  #[error(transparent)]
  UnresolvedImport(Box<UnresolvedImport>),
  // TODO: probably should remove this error
  #[error("Napi error: {status}: {reason}")]
  Napi { status: String, reason: String },
}

impl BuildError {
  pub fn code(&self) -> &'static str {
    match self {
      Self::UnresolvedEntry(_) | Self::ExternalEntry(_) => error_code::UNRESOLVED_ENTRY,
      Self::UnresolvedImport(_) => error_code::UNRESOLVED_IMPORT,
      Self::Napi { .. } => todo!(),
    }
  }

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
