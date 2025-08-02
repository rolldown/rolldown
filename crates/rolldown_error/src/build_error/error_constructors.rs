use std::path::Path;

use super::BuildDiagnostic;
use super::severity::Severity;
use arcstr::ArcStr;
use oxc::diagnostics::OxcDiagnostic;
use oxc::{diagnostics::LabeledSpan, span::{Span, CompactStr}};
use oxc_resolver::ResolveError;

use crate::events::DiagnosableArcstr;
#[cfg(feature = "napi")]
use crate::events::NapiError;
use crate::events::assign_to_import::AssignToImport;
use crate::events::configuration_field_conflict::ConfigurationFieldConflict;
use crate::events::export_undefined_variable::ExportUndefinedVariable;
use crate::events::filename_conflict::FilenameConflict;
use crate::events::illegal_identifier_as_name::IllegalIdentifierAsName;
use crate::events::import_is_undefined::ImportIsUndefined;
use crate::events::invalid_define_config::InvalidDefineConfig;
use crate::events::invalid_option::{InvalidOption, InvalidOptionType};
use crate::events::json_parse::JsonParse;
use crate::events::missing_global_name::MissingGlobalName;
use crate::events::missing_name_option_for_iife_export::MissingNameOptionForIifeExport;
use crate::events::missing_name_option_for_umd_export::MissingNameOptionForUmdExport;
use crate::events::resolve_error::DiagnosableResolveError;
use crate::events::unhandleable_error::UnhandleableError;
use crate::events::unloadable_dependency::{UnloadableDependency, UnloadableDependencyContext};
use crate::events::unsupported_feature::UnsupportedFeature;
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
  unresolved_entry::UnresolvedEntry,
};
use crate::line_column_to_byte_offset;

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

  pub fn resolve_error(
    source: ArcStr,
    importer_id: ArcStr,
    importee: DiagnosableArcstr,
    reason: String,
    diagnostic_kind: crate::event_kind::EventKind,
    help: Option<String>,
  ) -> Self {
    Self::new_inner(DiagnosableResolveError {
      source,
      importer_id,
      importee,
      reason,
      help,
      diagnostic_kind,
    })
  }

  pub fn unloadable_dependency(
    resolved: ArcStr,
    context: Option<UnloadableDependencyContext>,
    err: anyhow::Error,
  ) -> Self {
    downcast_napi_error_diagnostics(err).unwrap_or_else(|err| {
      Self::new_inner(UnloadableDependency { resolved, context, reason: err.to_string().into() })
    })
  }

  pub fn circular_dependency(paths: Vec<String>) -> Self {
    Self::new_inner(CircularDependency { paths })
  }

  pub fn missing_export(
    importer: String,
    stable_importer: String,
    stable_importee: String,
    importer_source: ArcStr,
    imported_specifier: String,
    imported_specifier_span: Span,
  ) -> Self {
    Self::new_inner(MissingExport {
      importer,
      stable_importer,
      stable_importee,
      importer_source,
      imported_specifier: imported_specifier.into(),
      imported_specifier_span,
    })
  }

  pub fn mixed_export(
    module_id: String,
    module_name: ArcStr,
    entry_module: String,
    export_keys: Vec<ArcStr>,
  ) -> Self {
    Self::new_inner(MixedExport { module_id, module_name, entry_module, export_keys })
  }

  pub fn missing_global_name(module_id: String, module_name: ArcStr, guessed_name: CompactStr) -> Self {
    Self::new_inner(MissingGlobalName { module_id, module_name, guessed_name })
  }

  pub fn missing_name_option_for_iife_export() -> Self {
    Self::new_inner(MissingNameOptionForIifeExport {})
  }

  pub fn missing_name_option_for_umd_export() -> Self {
    Self::new_inner(MissingNameOptionForUmdExport {})
  }

  pub fn illegal_identifier_as_name(identifier_name: CompactStr) -> Self {
    Self::new_inner(IllegalIdentifierAsName { identifier_name })
  }

  pub fn invalid_export_option(
    export_mode: CompactStr,
    entry_module: ArcStr,
    export_keys: Vec<CompactStr>,
  ) -> Self {
    Self::new_inner(InvalidExportOption {
      export_mode,
      entry_module,
      export_keys,
    })
  }

  pub fn filename_conflict(filename: ArcStr) -> Self {
    Self::new_inner(FilenameConflict { filename })
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
    name: CompactStr,
    stable_importer: String,
  ) -> Self {
    Self::new_inner(ImportIsUndefined { filename, source, span, name, stable_importer })
  }

  pub fn unsupported_feature(
    filename: ArcStr,
    source: ArcStr,
    span: Span,
    error_message: String,
  ) -> Self {
    Self::new_inner(UnsupportedFeature { filename, source, span, error_message })
  }

  pub fn empty_import_meta(filename: String, source: ArcStr, span: Span, format: CompactStr) -> Self {
    Self::new_inner(crate::events::empty_import_meta::EmptyImportMeta {
      filename,
      source,
      span,
      format,
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

  pub fn from_oxc_diagnostics<T>(
    diagnostics: T,
    source: &ArcStr,
    path: &str,
    severity: &Severity,
  ) -> Vec<Self>
  where
    T: IntoIterator<Item = OxcDiagnostic>,
  {
    diagnostics
      .into_iter()
      .map(|mut error| {
        let diagnostic = BuildDiagnostic::oxc_parse_error(
          source.clone(),
          path.to_string(),
          error.help.take().unwrap_or_default().into(),
          error.message.to_string(),
          error.labels.take().unwrap_or_default(),
        );
        if matches!(severity, Severity::Warning) {
          diagnostic.with_severity_warning()
        } else {
          diagnostic
        }
      })
      .collect::<Vec<_>>()
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

  pub fn invalid_option(invalid_option_type: InvalidOptionType) -> Self {
    Self::new_inner(InvalidOption { invalid_option_type })
  }

  #[cfg(feature = "napi")]
  pub fn napi_error(err: napi::Error) -> Self {
    let mut diagnostic = Self::new_inner(NapiError {});
    diagnostic.napi_error = Some(err);
    diagnostic
  }

  pub fn eval(filename: String, source: ArcStr, span: Span) -> Self {
    Self::new_inner(Eval { filename, span, source })
  }

  pub fn configuration_field_conflict(
    a_config_name: &str,
    a_field_name: &str,
    b_config_name: &str,
    b_field_name: &str,
  ) -> Self {
    Self::new_inner(ConfigurationFieldConflict {
      a_field: a_field_name.to_string(),
      a_config_name: a_config_name.to_string(),
      b_field: b_field_name.to_string(),
      b_config_name: b_config_name.to_string(),
    })
  }

  pub fn export_undefined_variable(
    filename: String,
    source: ArcStr,
    span: Span,
    name: CompactStr,
  ) -> Self {
    Self::new_inner(ExportUndefinedVariable { filename, source, span, name })
  }

  pub fn assign_to_import(filename: ArcStr, source: ArcStr, span: Span, name: CompactStr) -> Self {
    Self::new_inner(AssignToImport { filename, source, span, name })
  }

  #[allow(clippy::cast_possible_truncation)]
  pub fn json_parse(
    filename: ArcStr,
    source: ArcStr,
    line: usize,
    column: usize,
    message: ArcStr,
  ) -> Self {
    // `serde_json` Error is one-based https://docs.rs/serde_json/1.0.132/serde_json/struct.Error.html#method.column
    let start_offset = line_column_to_byte_offset(source.as_str(), line - 1, column - 1);
    let span = Span::new(start_offset as u32, start_offset as u32);
    Self::new_inner(JsonParse { filename, source, span, message })
  }

  pub fn invalid_define_config(message: String) -> Self {
    Self::new_inner(InvalidDefineConfig { message })
  }

  pub fn unhandleable_error(err: anyhow::Error) -> Self {
    downcast_napi_error_diagnostics(err)
      .unwrap_or_else(|err| Self::new_inner(UnhandleableError(err)))
  }
}

fn downcast_napi_error_diagnostics(err: anyhow::Error) -> Result<BuildDiagnostic, anyhow::Error> {
  #[cfg(feature = "napi")]
  {
    err.downcast::<napi::Error>().map(BuildDiagnostic::napi_error)
  }
  #[cfg(not(feature = "napi"))]
  {
    Err(err)
  }
}
