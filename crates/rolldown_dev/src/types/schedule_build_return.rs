use crate::dev_context::BundlingFuture;

/// Return value for `schedule_build_if_stale` indicating whether a build was scheduled
#[derive(Debug, Clone)]
pub struct ScheduleBuildReturn {
  /// The bundling task future
  pub future: BundlingFuture,
  #[expect(unused)]
  pub is_previous_task: bool,
}
