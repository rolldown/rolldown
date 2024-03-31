use std::{
  borrow::Cow,
  fmt::Display,
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc::span::Span;

use crate::{
  diagnostic::Diagnostic,
  error_kind::{
    external_entry::ExternalEntry, forbid_const_assign::ForbidConstAssign,
    sourcemap_error::SourceMapError, unresolved_entry::UnresolvedEntry,
    unresolved_import::UnresolvedImport, unsupported_eval::UnsupportedEval, BuildErrorLike,
    NapiError,
  },
};

type StaticStr = Cow<'static, str>;

#[derive(Debug, Clone)]
pub enum Severity {
  Error,
  Warning,
}

#[derive(Debug)]
pub struct BuildError {
  inner: Box<dyn BuildErrorLike>,
  source: Option<Box<dyn std::error::Error + 'static + Send + Sync>>,
  severity: Severity,
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

  #[must_use]
  pub fn with_severity_warning(mut self) -> Self {
    self.severity = Severity::Warning;
    self
  }

  pub fn into_diagnostic(self) -> Diagnostic {
    let mut builder = self.inner.diagnostic_builder();
    builder.severity = Some(self.severity);
    builder.build()
  }

  // --- private

  fn new_inner(inner: impl Into<Box<dyn BuildErrorLike>>) -> Self {
    Self { inner: inner.into(), source: None, severity: Severity::Error }
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

  pub fn sourcemap_error(error: oxc::sourcemap::Error) -> Self {
    Self::new_inner(SourceMapError { error })
  }

  // --- rolldown specific
  pub fn napi_error(status: String, reason: String) -> Self {
    Self::new_inner(NapiError { status, reason })
  }

  pub fn unsupported_eval(filename: String, source: Arc<str>, span: Span) -> Self {
    Self::new_inner(UnsupportedEval { filename, eval_span: span, source })
  }

  pub fn forbid_const_assign(
    filename: String,
    source: Arc<str>,
    name: String,
    reference_span: Span,
    re_assign_span: Span,
  ) -> Self {
    Self::new_inner(ForbidConstAssign { filename, source, name, reference_span, re_assign_span })
  }
}

impl From<std::io::Error> for BuildError {
  fn from(e: std::io::Error) -> Self {
    Self::new_inner(e)
  }
}

#[cfg(feature = "napi")]
impl From<napi::Error> for BuildError {
  fn from(e: napi::Error) -> Self {
    BuildError::napi_error(e.status.to_string(), e.reason)
  }
}
