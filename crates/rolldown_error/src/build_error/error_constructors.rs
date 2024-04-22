use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use oxc::span::Span;

use super::BuildError;

use crate::events::{
  circular_dependency::CircularDependency, external_entry::ExternalEntry,
  forbid_const_assign::ForbidConstAssign, sourcemap_error::SourceMapError,
  unresolved_entry::UnresolvedEntry, unresolved_import::UnresolvedImport,
  unsupported_eval::UnsupportedEval, NapiError,
};

impl BuildError {
  // --- Rollup related
  pub fn entry_cannot_be_external(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_inner(ExternalEntry { id: unresolved_id.as_ref().to_path_buf() })
  }

  pub fn unresolved_entry(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_inner(UnresolvedEntry { unresolved_id: unresolved_id.as_ref().to_path_buf() })
  }

  pub fn unresolved_import(specifier: impl Into<String>, importer: impl Into<PathBuf>) -> Self {
    Self::new_inner(UnresolvedImport { specifier: specifier.into(), importer: importer.into() })
  }

  pub fn sourcemap_error(error: oxc::sourcemap::Error) -> Self {
    Self::new_inner(SourceMapError { error })
  }

  pub fn circular_dependency(paths: Vec<String>) -> Self {
    Self::new_inner(CircularDependency { paths })
  }

  // --- Rolldown related

  pub fn forbid_const_assign(
    filename: String,
    source: Arc<str>,
    name: String,
    reference_span: Span,
    re_assign_span: Span,
  ) -> Self {
    Self::new_inner(ForbidConstAssign { filename, source, name, reference_span, re_assign_span })
  }

  pub fn napi_error(status: String, reason: String) -> Self {
    Self::new_inner(NapiError { status, reason })
  }

  pub fn unsupported_eval(filename: String, source: Arc<str>, span: Span) -> Self {
    Self::new_inner(UnsupportedEval { filename, eval_span: span, source })
  }
}
