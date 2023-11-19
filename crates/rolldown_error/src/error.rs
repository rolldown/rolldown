use std::{
  borrow::Cow,
  fmt::Display,
  path::{Path, PathBuf},
};

use crate::error_kind::{
  external_entry::ExternalEntry, unresolved_entry::UnresolvedEntry,
  unresolved_import::UnresolvedImport, BuildErrorLike, NapiError,
};

type StaticStr = Cow<'static, str>;

#[derive(Debug)]
pub struct BuildError {
  inner: Box<dyn BuildErrorLike>,
  source: Option<Box<dyn std::error::Error + 'static + Send + Sync>>,
}

fn _assert_build_error_send_sync() {
  fn _assert_send_sync<T: Send + Sync>() {}
  _assert_send_sync::<BuildError>();
}

impl Display for BuildError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.inner.message().fmt(f)
  }
}

impl std::error::Error for BuildError {
  // clippy::option_map_or_none: Cool. Finally, catch a error of clippy. Clippy suggest using `self.source.as_ref().map(|source| source.as_ref())`
  // which will cause type mismatch error.
  #[allow(clippy::option_map_or_none)]
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    self.source.as_ref().map_or(None, |source| Some(source.as_ref()))
  }
}

impl BuildError {
  pub fn code(&self) -> &'static str {
    self.inner.code()
  }

  #[must_use]
  pub fn with_source(
    mut self,
    source: impl Into<Box<dyn std::error::Error + 'static + Send + Sync>>,
  ) -> Self {
    self.source = Some(source.into());
    self
  }

  // --- private

  fn new_inner(inner: impl Into<Box<dyn BuildErrorLike>>) -> Self {
    Self { inner: inner.into(), source: None }
  }

  // --- Aligned with rollup
  pub fn entry_cannot_be_external(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_inner(ExternalEntry { id: unresolved_id.as_ref().to_path_buf() })
  }

  pub fn unresolved_entry(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_inner(UnresolvedEntry { unresolved_id: unresolved_id.as_ref().to_path_buf() })
  }

  pub fn unresolved_import(specifier: impl Into<StaticStr>, importer: impl Into<PathBuf>) -> Self {
    Self::new_inner(UnresolvedImport { specifier: specifier.into(), importer: importer.into() })
  }

  // --- rolldown specific
  pub fn napi_error(status: String, reason: String) -> Self {
    Self::new_inner(NapiError { status, reason })
  }
}

impl From<std::io::Error> for BuildError {
  fn from(e: std::io::Error) -> Self {
    Self::new_inner(e)
  }
}
