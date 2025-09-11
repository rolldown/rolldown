mod build_error;
mod diagnostic;
mod event_kind;
mod events;
mod generated;
mod locator;
mod types;
mod utils;

use crate::build_error::BatchedBuildDiagnostic;

pub type BuildResult<T> = Result<T, BatchedBuildDiagnostic>;
pub type SingleBuildResult<T> = std::result::Result<T, BuildDiagnostic>;

pub use crate::{
  build_error::{BuildDiagnostic, severity::Severity},
  event_kind::EventKind,
  events::DiagnosableArcstr,
  events::ambiguous_external_namespace::AmbiguousExternalNamespaceModule,
  events::bundler_initialize_error::BundlerInitializeError,
  events::commonjs_variable_in_esm::CjsExportSpan,
  events::invalid_option::InvalidOptionType,
  events::unhandleable_error::CausedPlugin,
  events::unloadable_dependency::UnloadableDependencyContext,
  generated::event_kind_switcher::EventKindSwitcher,
  locator::line_column_to_byte_offset,
  types::diagnostic_options::DiagnosticOptions,
  types::result_ext::ResultExt,
  utils::filter_out_disabled_diagnostics,
};

fn _usage_should_able_to_auto_convert_outside_errors() -> BuildResult<()> {
  let _: usize = 0u32.try_into().map_err_to_unhandleable()?;
  Ok(())
}

fn _assert_build_error_send_sync() {
  fn assert_send_sync<T: Send + Sync>() {}
  assert_send_sync::<BuildDiagnostic>();
}
