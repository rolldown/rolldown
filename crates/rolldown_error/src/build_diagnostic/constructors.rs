use std::path::Path;

use arcstr::ArcStr;
use oxc::diagnostics::OxcDiagnostic;
use oxc::{diagnostics::LabeledSpan, span::Span};
use oxc_resolver::ResolveError;

use crate::utils::ByteLocator;

#[cfg(feature = "napi")]
use super::events::napi_error::NapiError;

use super::BuildDiagnostic;
use super::Severity;
use super::events::DiagnosableArcstr;
use super::events::assign_to_import::AssignToImport;
use super::events::bundler_initialize_error::BundlerInitializeError;
use super::events::configuration_field_conflict::ConfigurationFieldConflict;
use super::events::export_undefined_variable::ExportUndefinedVariable;
use super::events::filename_conflict::FilenameConflict;
use super::events::illegal_identifier_as_name::IllegalIdentifierAsName;
use super::events::import_is_undefined::ImportIsUndefined;
use super::events::invalid_define_config::InvalidDefineConfig;
use super::events::invalid_option::{InvalidOption, InvalidOptionType};
use super::events::json_parse::JsonParse;
use super::events::missing_global_name::MissingGlobalName;
use super::events::missing_name_option_for_iife_export::MissingNameOptionForIifeExport;
use super::events::missing_name_option_for_umd_export::MissingNameOptionForUmdExport;
use super::events::plugin_error::{CausedPlugin, PluginError};
use super::events::prefer_builtin_feature::PreferBuiltinFeature;
use super::events::resolve_error::DiagnosableResolveError;
use super::events::unhandleable_error::UnhandleableError;
use super::events::unloadable_dependency::{UnloadableDependency, UnloadableDependencyContext};
use super::events::unsupported_feature::UnsupportedFeature;
use super::events::{
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
    diagnostic_kind: crate::types::event_kind::EventKind,
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
    reason: ArcStr,
  ) -> Self {
    Self::new_inner(UnloadableDependency { resolved, context, reason })
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
    note: Option<String>,
  ) -> Self {
    Self::new_inner(MissingExport {
      importer,
      stable_importer,
      stable_importee,
      importer_source,
      imported_specifier,
      imported_specifier_span,
      note,
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

  pub fn missing_global_name(module_id: String, module_name: ArcStr, guessed_name: ArcStr) -> Self {
    Self::new_inner(MissingGlobalName { module_id, module_name, guessed_name })
  }

  pub fn missing_name_option_for_iife_export() -> Self {
    Self::new_inner(MissingNameOptionForIifeExport {})
  }

  pub fn missing_name_option_for_umd_export() -> Self {
    Self::new_inner(MissingNameOptionForUmdExport {})
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
    name: ArcStr,
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

  pub fn empty_import_meta(filename: String, source: ArcStr, span: Span, format: ArcStr) -> Self {
    Self::new_inner(super::events::empty_import_meta::EmptyImportMeta {
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
    source: ArcStr,
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
    Self::new_inner(NapiError(err))
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
    name: ArcStr,
  ) -> Self {
    Self::new_inner(ExportUndefinedVariable { filename, source, span, name })
  }

  pub fn assign_to_import(filename: ArcStr, source: ArcStr, span: Span, name: ArcStr) -> Self {
    Self::new_inner(AssignToImport { filename, source, span, name })
  }

  pub fn prefer_builtin_feature(builtin_feature: Option<String>, plugin_name: String) -> Self {
    Self::new_inner(PreferBuiltinFeature { builtin_feature, plugin_name })
  }

  #[expect(clippy::cast_possible_truncation)]
  pub fn json_parse(
    filename: ArcStr,
    source: ArcStr,
    line: usize,
    column: usize,
    message: ArcStr,
  ) -> Self {
    // `serde_json` Error is one-based https://docs.rs/serde_json/1.0.132/serde_json/struct.Error.html#method.column
    let offset = ByteLocator::new(source.as_str()).byte_offset(line - 1, column - 1);
    let span = Span::new(offset as u32, offset as u32);
    Self::new_inner(JsonParse { filename, source, span, message })
  }

  pub fn invalid_define_config(message: String) -> Self {
    Self::new_inner(InvalidDefineConfig { message })
  }

  pub fn unhandleable_error(err: anyhow::Error) -> Self {
    Self::new_inner(UnhandleableError(err))
  }

  pub fn bundler_initialize_error(message: String, hint: Option<String>) -> Self {
    Self::new_inner(BundlerInitializeError { message, hint })
  }

  pub fn plugin_error(caused_plugin: CausedPlugin, err: anyhow::Error) -> Self {
    Self::new_inner(PluginError { plugin: caused_plugin, error: err })
  }
}
