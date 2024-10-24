mod build_error;
mod diagnostic;
mod event_kind;
mod events;
mod type_aliases;
mod types;

pub use types::result_ext::ResultExt;

pub use crate::{
  build_error::{BuildDiagnostic, BuildResult},
  event_kind::EventKind,
  events::ambiguous_external_namespace::AmbiguousExternalNamespaceModule,
  events::commonjs_variable_in_esm::CjsExportSpan,
  events::invalid_option::InvalidOptionTypes,
  events::unloadable_dependency::UnloadableDependencyContext,
  events::DiagnosableArcstr,
  type_aliases::{DiagnosableResult, UnhandleableResult},
  types::diagnostic_options::DiagnosticOptions,
};

fn _usage_should_able_to_auto_convert_outside_errors() -> DiagnosableResult<()> {
  let _: usize = 0u32.try_into().map_error_to_unhandleable()?;
  Ok(())
}
