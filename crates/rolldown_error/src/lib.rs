mod build_diagnostic;
mod generated;
mod types;
mod utils;

use crate::build_diagnostic::BatchedBuildDiagnostic;

pub type BuildResult<T> = Result<T, BatchedBuildDiagnostic>;
pub type SingleBuildResult<T> = std::result::Result<T, BuildDiagnostic>;

pub use crate::{
  build_diagnostic::events::DiagnosableArcstr,
  build_diagnostic::events::ambiguous_external_namespace::AmbiguousExternalNamespaceModule,
  build_diagnostic::events::bundler_initialize_error::BundlerInitializeError,
  build_diagnostic::events::commonjs_variable_in_esm::CjsExportSpan,
  build_diagnostic::events::invalid_option::InvalidOptionType,
  build_diagnostic::events::unhandleable_error::CausedPlugin,
  build_diagnostic::events::unloadable_dependency::UnloadableDependencyContext,
  build_diagnostic::{BuildDiagnostic, Severity},
  generated::event_kind_switcher::EventKindSwitcher,
  types::diagnostic_options::DiagnosticOptions,
  types::event_kind::EventKind,
  utils::ResultExt,
  utils::downcast_napi_error_diagnostics,
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
