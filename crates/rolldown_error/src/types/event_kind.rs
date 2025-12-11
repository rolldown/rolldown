//! Naming convention:
//! - All kinds that will terminate the build process should be named with a postfix "Error".
use std::fmt::Display;

#[derive(Clone, Copy)]
pub enum EventKind {
  // --- These kinds are copied from rollup: https://github.com/rollup/rollup/blob/0b665c31833525c923c0fc20f43ebfca748c6670/src/utils/logs.ts#L102-L179
  AmbiguousExternalNamespaceError = 0,
  /// Whether to emit warning when detecting circular dependency
  AmbiguousReexportError = 35,
  CircularDependency = 1,
  CircularReexportError = 2,
  Eval = 3,
  IllegalIdentifierAsNameError = 4,
  InvalidExportOptionError = 5,
  InvalidOptionError = 6,
  MissingExportError = 7,
  MissingGlobalName = 8,
  MissingNameOptionForIifeExport = 9,
  MissingNameOptionForUmdExportError = 10,
  MixedExport = 11,
  ParseError = 12,
  UnresolvedEntry = 13,
  UnresolvedImport = 14,
  FilenameConflict = 15,
  // !! Only add new kind if it's not covered by the kinds from rollup !!

  // --- These kinds are derived from esbuild
  AssignToImportError = 16,
  CommonJsVariableInEsm = 17,
  ExportUndefinedVariableError = 18,
  ImportIsUndefined = 19,
  UnsupportedFeatureError = 20,
  EmptyImportMeta = 21,

  // --- These kinds are rolldown specific
  JsonParseError = 22,
  IllegalReassignmentError = 23,
  InvalidDefineConfigError = 24,
  ResolveError = 25,
  UnhandleableError = 26,
  UnloadableDependencyError = 27,

  NapiError = 28,
  ConfigurationFieldConflict = 29,
  PreferBuiltinFeature = 30,
  BundlerInitializeError = 31,
  PluginError = 32,
  AlreadyClosedError = 33,
  CouldNotCleanDirectory = 34,
}

impl Display for EventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // --- Copied from rollup
      EventKind::AmbiguousExternalNamespaceError => write!(f, "AMBIGUOUS_EXTERNAL_NAMESPACES"),
      EventKind::AmbiguousReexportError => write!(f, "AMBIGUOUS_REEXPORT"),
      EventKind::CircularDependency => write!(f, "CIRCULAR_DEPENDENCY"),
      EventKind::CircularReexportError => write!(f, "CIRCULAR_REEXPORT"),
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
      EventKind::EmptyImportMeta => write!(f, "EMPTY_IMPORT_META"),

      // --- Rolldown specific
      EventKind::JsonParseError => write!(f, "JSON_PARSE"),
      EventKind::IllegalReassignmentError => write!(f, "ILLEGAL_REASSIGNMENT"),
      EventKind::InvalidDefineConfigError => write!(f, "INVALID_DEFINE_CONFIG"),
      EventKind::ResolveError => write!(f, "RESOLVE_ERROR"),
      EventKind::UnhandleableError => write!(f, "UNHANDLEABLE_ERROR"),
      EventKind::UnloadableDependencyError => write!(f, "UNLOADABLE_DEPENDENCY"),

      EventKind::NapiError => write!(f, "NAPI_ERROR"),
      EventKind::ConfigurationFieldConflict => write!(f, "CONFIGURATION_FIELD_CONFLICT"),
      EventKind::PreferBuiltinFeature => write!(f, "PREFER_BUILTIN_FEATURE"),
      EventKind::BundlerInitializeError => write!(f, "BUNDLER_INITIALIZE_ERROR"),
      EventKind::PluginError => write!(f, "PLUGIN_ERROR"),
      EventKind::AlreadyClosedError => write!(f, "ALREADY_CLOSED"),
      EventKind::CouldNotCleanDirectory => write!(f, "COULD_NOT_CLEAN_DIRECTORY"),
    }
  }
}
