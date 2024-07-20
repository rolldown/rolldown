use std::path::{Path, PathBuf};

use arcstr::ArcStr;
use oxc::{diagnostics::LabeledSpan, span::Span};
use rolldown_resolver::ResolveError;

use super::BuildDiagnostic;

use crate::events::{
  ambiguous_external_namespace::{AmbiguousExternalNamespace, AmbiguousExternalNamespaceModule},
  circular_dependency::CircularDependency,
  eval::Eval,
  external_entry::ExternalEntry,
  forbid_const_assign::ForbidConstAssign,
  missing_export::MissingExport,
  parse_error::ParseError,
  sourcemap_error::SourceMapError,
  unresolved_entry::UnresolvedEntry,
  unresolved_import::UnresolvedImport,
  unresolved_import_treated_as_external::UnresolvedImportTreatedAsExternal,
  NapiError,
};

impl BuildDiagnostic {
  // --- Rollup related
  pub fn entry_cannot_be_external(unresolved_id: impl AsRef<Path>) -> Self {
    Self::new_inner(ExternalEntry { id: unresolved_id.as_ref().to_path_buf() })
  }

  pub fn ambiguous_external_namespace(
    ambiguous_export_name: String,
    importee: String,
    importer: AmbiguousExternalNamespaceModule,
    exporter: Vec<AmbiguousExternalNamespaceModule>,
  ) -> Self {
    Self::new_inner(AmbiguousExternalNamespace {
      ambiguous_export_name,
      importee,
      importer,
      exporter,
    })
  }

  pub fn unresolved_entry(
    unresolved_id: impl AsRef<Path>,
    resolve_error: Option<ResolveError>,
  ) -> Self {
    Self::new_inner(UnresolvedEntry {
      unresolved_id: unresolved_id.as_ref().to_path_buf(),
      resolve_error,
    })
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

  pub fn unresolved_import_treated_as_external(
    specifier: impl Into<String>,
    importer: impl Into<PathBuf>,
    resolve_error: Option<ResolveError>,
  ) -> Self {
    Self::new_inner(UnresolvedImportTreatedAsExternal {
      specifier: specifier.into(),
      importer: importer.into(),
      resolve_error,
    })
  }

  pub fn missing_export(
    stable_importer: String,
    stable_importee: String,
    importer_source: ArcStr,
    imported_specifier: String,
    imported_specifier_span: Span,
  ) -> Self {
    Self::new_inner(MissingExport {
      stable_importer,
      stable_importee,
      importer_source,
      imported_specifier,
      imported_specifier_span,
    })
  }

  // --- Rolldown related

  pub fn oxc_parse_error(
    source: ArcStr,
    filename: String,
    error_help: String,
    error_message: String,
    error_labels: Vec<LabeledSpan>,
  ) -> Self {
    Self::new_inner(ParseError { source, filename, error_help, error_message, error_labels })
  }

  pub fn forbid_const_assign(
    filename: String,
    source: ArcStr,
    name: String,
    reference_span: Span,
    re_assign_span: Span,
  ) -> Self {
    Self::new_inner(ForbidConstAssign { filename, source, name, reference_span, re_assign_span })
  }

  pub fn napi_error(status: String, reason: String) -> Self {
    Self::new_inner(NapiError { status, reason })
  }

  pub fn eval(filename: String, source: ArcStr, span: Span) -> Self {
    Self::new_inner(Eval { filename, span, source })
  }
}
