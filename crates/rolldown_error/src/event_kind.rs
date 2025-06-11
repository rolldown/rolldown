//! Naming convention:
//! - All kinds that will terminate the build process should be named with a postfix "Error".
use std::{f32::consts::E, fmt::Display};

#[derive(Clone, Copy)]
pub enum EventKind {
  // --- These kinds are copied from rollup: https://github.com/rollup/rollup/blob/0b665c31833525c923c0fc20f43ebfca748c6670/src/utils/logs.ts#L102-L179
  AmbiguousExternalNamespaceError = 0,
  /// Whether to emit warning when detecting circular dependency
  CircularDependency = 1,
  Eval = 2,
  IllegalIdentifierAsNameError = 3,
  InvalidExportOptionError = 4,
  InvalidOptionError = 5,
  MissingExportError = 6,
  MissingGlobalName = 7,
  MissingNameOptionForIifeExport = 8,
  MissingNameOptionForUmdExportError = 9,
  MixedExport = 10,
  ParseError = 11,
  UnresolvedEntry = 12,
  UnresolvedImport = 13,
  FilenameConflict = 14,
  // !! Only add new kind if it's not covered by the kinds from rollup !!

  // --- These kinds are derived from esbuild
  AssignToImportError = 15,
  CommonJsVariableInEsm = 16,
  ExportUndefinedVariableError = 17,
  ImportIsUndefined = 18,
  UnsupportedFeatureError = 19,

  // --- These kinds are rolldown specific
  JsonParseError = 20,
  IllegalReassignmentError = 21,
  InvalidDefineConfigError = 22,
  ResolveError = 23,
  UnhandleableError = 24,
  UnloadableDependencyError = 25,

  IoError = 26,
  NapiError = 27,
  ConfigurationFieldConflict = 28,
  UnsupportedTarget = 29,
}

impl Display for EventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // --- Copied from rollup
      EventKind::AmbiguousExternalNamespaceError => write!(f, "AMBIGUOUS_EXTERNAL_NAMESPACES"),
      EventKind::CircularDependency => write!(f, "CIRCULAR_DEPENDENCY"),
      EventKind::Eval => write!(f, "EVAL"),
      EventKind::IllegalIdentifierAsNameError => write!(f, "ILLEGAL_IDENTIFIER_AS_NAME"),
      EventKind::InvalidExportOptionError => write!(f, "INVALID_EXPORT_OPTION"),
      EventKind::InvalidOptionError => write!(f, "INVALID_OPTION"),
      EventKind::MixedExport => write!(f, "MIXED_EXPORT"),
      EventKind::MissingGlobalName => write!(f, "MISSING_GLOBAL_NAME"),
      EventKind::MissingNameOptionForIifeExport => write!(f, "MISSING_NAME_OPTION_FOR_IIFE_EXPORT"),
      EventKind::MissingNameOptionForUmdExportError => {
        write!(f, "MISSING_NAME_OPTION_FOR_UMD_EXPORT")
      }
      EventKind::MissingExportError => write!(f, "MISSING_EXPORT"),
      EventKind::ParseError => write!(f, "PARSE_ERROR"),
      EventKind::UnresolvedEntry => write!(f, "UNRESOLVED_ENTRY"),
      EventKind::UnresolvedImport => write!(f, "UNRESOLVED_IMPORT"),
      EventKind::FilenameConflict => write!(f, "FILE_NAME_CONFLICT"),

      // --- Derived from esbuild
      EventKind::AssignToImportError => write!(f, "ASSIGN_TO_IMPORT"),
      EventKind::CommonJsVariableInEsm => write!(f, "COMMONJS_VARIABLE_IN_ESM"),
      EventKind::ExportUndefinedVariableError => write!(f, "EXPORT_UNDEFINED_VARIABLE"),
      EventKind::ImportIsUndefined => write!(f, "IMPORT_IS_UNDEFINED"),
      EventKind::UnsupportedFeatureError => write!(f, "UNSUPPORTED_FEATURE"),

      // --- Rolldown specific
      EventKind::JsonParseError => write!(f, "JSON_PARSE"),
      EventKind::IllegalReassignmentError => write!(f, "ILLEGAL_REASSIGNMENT"),
      EventKind::InvalidDefineConfigError => write!(f, "INVALID_DEFINE_CONFIG"),
      EventKind::ResolveError => write!(f, "RESOLVE_ERROR"),
      EventKind::UnhandleableError => write!(f, "UNHANDLEABLE_ERROR"),
      EventKind::UnloadableDependencyError => write!(f, "UNLOADABLE_DEPENDENCY"),

      EventKind::IoError => write!(f, "IO_ERROR"),
      EventKind::NapiError => write!(f, "NAPI_ERROR"),
      EventKind::ConfigurationFieldConflict => write!(f, "CONFIGURATION_FIELD_CONFLICT"),
      EventKind::UnsupportedTarget => write!(f, "UNSUPPORTED_TARGET"),
    }
  }
}
