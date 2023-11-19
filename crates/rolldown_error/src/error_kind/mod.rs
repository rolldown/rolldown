use std::borrow::Cow;

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
pub enum ErrorKind {
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
