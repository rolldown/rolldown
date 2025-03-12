mod build_error;
mod diagnostic;
mod event_kind;
mod events;
mod generated;
mod locator;
mod type_aliases;
mod types;
mod utils;

pub use types::result_ext::ResultExt;

pub use crate::{
  build_error::{BuildDiagnostic, severity::Severity},
  event_kind::EventKind,
  events::DiagnosableArcstr,
  events::ambiguous_external_namespace::AmbiguousExternalNamespaceModule,
  events::commonjs_variable_in_esm::CjsExportSpan,
  events::invalid_option::InvalidOptionType,
  events::unloadable_dependency::UnloadableDependencyContext,
  locator::line_column_to_byte_offset,
  type_aliases::{BuildResult, SingleBuildResult},
  types::diagnostic_options::DiagnosticOptions,
};

fn _usage_should_able_to_auto_convert_outside_errors() -> BuildResult<()> {
  let _: usize = 0u32.try_into().map_err_to_unhandleable()?;
  Ok(())
}
