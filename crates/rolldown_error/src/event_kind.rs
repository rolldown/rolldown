//! Naming convention:
//! - All kinds that will terminate the build process should be named with a postfix "Error".
use std::fmt::Display;

pub enum EventKind {
  // --- These kinds are copied from rollup: https://github.com/rollup/rollup/blob/0b665c31833525c923c0fc20f43ebfca748c6670/src/utils/logs.ts#L102-L179
  AmbiguousExternalNamespaceError,
  CircularDependency,
  Eval,
  IllegalIdentifierAsNameError,
  InvalidExportOptionError,
  InvalidOptionError,
  MissingExportError,
  MissingGlobalName,
  MissingNameOptionForIifeExport,
  MissingNameOptionForUmdExportError,
  MixedExport,
  ParseError,
  SourcemapError,
  UnresolvedEntry,
  UnresolvedImport,
  FilenameConflict,
  // !! Only add new kind if it's not covered by the kinds from rollup !!

  // --- These kinds are derived from esbuild
  AssignToImportError,
  CommonJsVariableInEsm,
  ExportUndefinedVariableError,
  ImportIsUndefined,
  UnsupportedFeatureError,

  // --- These kinds are rolldown specific
  JsonParseError,
  IllegalReassignmentError,
  InvalidDefineConfigError,
  ResolveError(Option<&'static str>),
  UnhandleableError,
  UnloadableDependencyError,

  // TODO remove following kinds
  IoError,
  NapiError,
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
      EventKind::SourcemapError => write!(f, "SOURCEMAP_ERROR"),
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
      EventKind::ResolveError(title) => match title {
        Some(title) => write!(f, "{title}"),
        None => write!(f, "RESOLVE_ERROR"),
      },
      EventKind::UnhandleableError => write!(f, "UNHANDLEABLE_ERROR"),
      EventKind::UnloadableDependencyError => write!(f, "UNLOADABLE_DEPENDENCY"),

      // TODO remove following kinds
      EventKind::IoError => write!(f, "IO_ERROR"),
      EventKind::NapiError => write!(f, "NAPI_ERROR"),
    }
  }
}
