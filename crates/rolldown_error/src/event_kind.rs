use std::fmt::Display;

pub enum EventKind {
  // --- These kinds are copied from rollup: https://github.com/rollup/rollup/blob/0b665c31833525c923c0fc20f43ebfca748c6670/src/utils/logs.ts#L102-L179
  UnresolvedEntry,
  UnresolvedImport,
  AmbiguousExternalNamespace,
  ParseError,

  Eval,
  CircularDependency,
  SourcemapError,
  MissingExport,
  // --- These kinds are rolldown specific
  IllegalReassignment,
  // !! Only add new kind if it's not covered by the kinds from rollup !!

  // TODO remove following kinds
  NapiError,
  IoError,
}

impl Display for EventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // --- Copied from rollup
      EventKind::UnresolvedEntry => write!(f, "UNRESOLVED_ENTRY"),
      EventKind::UnresolvedImport => write!(f, "UNRESOLVED_IMPORT"),
      EventKind::AmbiguousExternalNamespace => write!(f, "AMBIGUOUS_EXTERNAL_NAMESPACES"),
      EventKind::ParseError => write!(f, "PARSE_ERROR"),
      EventKind::IllegalReassignment => write!(f, "ILLEGAL_REASSIGNMENT"),
      EventKind::Eval => write!(f, "EVAL"),
      EventKind::SourcemapError => write!(f, "SOURCEMAP_ERROR"),
      EventKind::CircularDependency => write!(f, "CIRCULAR_DEPENDENCY"),
      EventKind::MissingExport => write!(f, "MISSING_EXPORT"),
      // --- Rolldown specific
      EventKind::NapiError => write!(f, "NAPI_ERROR"),
      EventKind::IoError => write!(f, "IO_ERROR"),
    }
  }
}
