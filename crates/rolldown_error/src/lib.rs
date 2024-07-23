mod build_error;
mod diagnostic;
mod event_kind;
mod events;
mod type_aliases;
mod types;

pub use crate::{
  build_error::{BuildDiagnostic, BuildResult},
  event_kind::EventKind,
  events::ambiguous_external_namespace::AmbiguousExternalNamespaceModule,
  type_aliases::{DiagnosableResult, UnhandleableResult},
  types::diagnostic_options::DiagnosticOptions,
};
