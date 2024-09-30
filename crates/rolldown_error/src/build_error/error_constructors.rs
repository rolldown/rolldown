use std::path::{Path, PathBuf};

use super::BuildDiagnostic;
use arcstr::ArcStr;
use oxc::{diagnostics::LabeledSpan, span::Span};
use rolldown_resolver::ResolveError;

use crate::events::export_undefined_variable::ExportUndefinedVariable;
use crate::events::illegal_identifier_as_name::IllegalIdentifierAsName;
use crate::events::import_is_undefined::ImportIsUndefined;
use crate::events::invalid_option::{InvalidOption, InvalidOptionTypes};
use crate::events::missing_global_name::MissingGlobalName;
use crate::events::missing_name_option_for_iife_export::MissingNameOptionForIifeExport;
use crate::events::resolve_error::DiagnosableResolveError;
use crate::events::unloadable_dependency::{UnloadableDependency, UnloadableDependencyContext};
use crate::events::{
  ambiguous_external_namespace::{AmbiguousExternalNamespace, AmbiguousExternalNamespaceModule},
  circular_dependency::CircularDependency,
  commonjs_variable_in_esm::{CjsExportSpan, CommonJsVariableInEsm},
  eval::Eval,
  external_entry::ExternalEntry,
  forbid_const_assign::ForbidConstAssign,
  invalid_export_option::InvalidExportOption,
  missing_export::MissingExport,
  mixed_export::MixedExport,
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

  pub fn diagnosable_resolve_error(
    source: ArcStr,
    importer_id: ArcStr,
    importee_span: Span,
    reason: String,
  ) -> Self {
    Self::new_inner(DiagnosableResolveError { source, importer_id, importee_span, reason })
  }

  pub fn unloadable_dependency(
    resolved: ArcStr,
    context: Option<UnloadableDependencyContext>,
    reason: ArcStr,
  ) -> Self {
    Self::new_inner(UnloadableDependency { resolved, context, reason })
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

  pub fn mixed_export(module_name: ArcStr, entry_module: ArcStr, export_keys: Vec<ArcStr>) -> Self {
    Self::new_inner(MixedExport { module_name, entry_module, export_keys })
  }

  pub fn missing_global_name(module_name: ArcStr, guessed_name: ArcStr) -> Self {
    Self::new_inner(MissingGlobalName { module_name, guessed_name })
  }

  pub fn missing_name_option_for_iife_export() -> Self {
    Self::new_inner(MissingNameOptionForIifeExport {})
  }

  pub fn illegal_identifier_as_name(identifier_name: ArcStr) -> Self {
    Self::new_inner(IllegalIdentifierAsName { identifier_name })
  }

  pub fn invalid_export_option(
    export_mode: ArcStr,
    entry_module: ArcStr,
    export_keys: Vec<ArcStr>,
  ) -> Self {
    Self::new_inner(InvalidExportOption { export_mode, export_keys, entry_module })
  }
  // Esbuild
  pub fn commonjs_variable_in_esm(
    filename: String,
    source: ArcStr,
    esm_export_span: Span,
    cjs_export_ident_span: CjsExportSpan,
  ) -> Self {
    Self::new_inner(CommonJsVariableInEsm {
      filename,
      source,
      esm_export_span,
      cjs_export_ident_span,
    })
  }

  pub fn import_is_undefined(
    filename: ArcStr,
    source: ArcStr,
    span: Span,
    name: ArcStr,
    stable_importer: String,
  ) -> Self {
    Self::new_inner(ImportIsUndefined { filename, source, span, name, stable_importer })
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

  pub fn invalid_option(situation: InvalidOptionTypes, option: String) -> Self {
    Self::new_inner(InvalidOption { invalid_option_types: situation, option })
  }

  pub fn napi_error(status: String, reason: String) -> Self {
    Self::new_inner(NapiError { status, reason })
  }

  pub fn eval(filename: String, source: ArcStr, span: Span) -> Self {
    Self::new_inner(Eval { filename, span, source })
  }

  pub fn export_undefined_variable(
    filename: String,
    source: ArcStr,
    span: Span,
    name: ArcStr,
  ) -> Self {
    Self::new_inner(ExportUndefinedVariable { filename, source, span, name })
  }
}
