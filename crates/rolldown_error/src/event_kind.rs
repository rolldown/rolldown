use std::fmt::Display;

pub enum EventKind {
  // --- These kinds are copied from rollup: https://github.com/rollup/rollup/blob/0b665c31833525c923c0fc20f43ebfca748c6670/src/utils/logs.ts#L102-L179
  UnresolvedEntry,
  UnresolvedImport,
  AmbiguousExternalNamespace,
  MixedExport,
  MissingGlobalName,
  MissingNameOptionForIifeExport,
  IllegalIdentifierAsName,
  ParseError,
  InvalidOption,

  Eval,
  CircularDependency,
  SourcemapError,
  MissingExport,
  InvalidExportOption,
  // --- These kinds are rolldown specific
  IllegalReassignment,
  UnloadableDependency,
  DiagnosableResolveError,
  // !! Only add new kind if it's not covered by the kinds from rollup !!

  // TODO remove following kinds
  NapiError,
  IoError,
  // Derive from esbuild
  CommonJsVariableInEsm,
  ExportUndefinedVariable,
}

impl Display for EventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // --- Copied from rollup
      EventKind::UnloadableDependency => write!(f, "UNLOADABLE_DEPENDENCY"),
      EventKind::UnresolvedEntry => write!(f, "UNRESOLVED_ENTRY"),
      EventKind::UnresolvedImport => write!(f, "UNRESOLVED_IMPORT"),
      EventKind::AmbiguousExternalNamespace => write!(f, "AMBIGUOUS_EXTERNAL_NAMESPACES"),
      EventKind::ParseError => write!(f, "PARSE_ERROR"),
      EventKind::IllegalReassignment => write!(f, "ILLEGAL_REASSIGNMENT"),
      EventKind::Eval => write!(f, "EVAL"),
      EventKind::SourcemapError => write!(f, "SOURCEMAP_ERROR"),
      EventKind::MixedExport => write!(f, "MIXED_EXPORT"),
      EventKind::MissingGlobalName => write!(f, "MISSING_GLOBAL_NAME"),
      EventKind::MissingNameOptionForIifeExport => write!(f, "MISSING_NAME_OPTION_FOR_IIFE_EXPORT"),
      EventKind::IllegalIdentifierAsName => write!(f, "ILLEGAL_IDENTIFIER_AS_NAME"),
      EventKind::CircularDependency => write!(f, "CIRCULAR_DEPENDENCY"),
      EventKind::MissingExport => write!(f, "MISSING_EXPORT"),
      EventKind::InvalidExportOption => write!(f, "INVALID_EXPORT_OPTION"),
      EventKind::InvalidOption => write!(f, "INVALID_OPTION"),
      // --- Rolldown specific
      EventKind::NapiError => write!(f, "NAPI_ERROR"),
      EventKind::IoError => write!(f, "IO_ERROR"),
      EventKind::CommonJsVariableInEsm => write!(f, "COMMONJS_VARIABLE_IN_ESM"),
      EventKind::ExportUndefinedVariable => write!(f, "EXPORT_UNDEFINED_VARIABLE"),
      EventKind::DiagnosableResolveError => write!(f, "DIAGNOSABLE_RESOLVE_ERROR"),
    }
  }
}
