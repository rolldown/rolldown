/// Which stage of `BundlingTask::run_inner` produced a build error.
///
/// Carried on `CoordinatorState::Failed` and used by `handle_file_changes`
/// to pick the right `TaskInput` on recovery: an `Hmr`-stage failure
/// recovers with an `Hmr` task (re-runs `watch_change` + HMR computation);
/// a `Rebuild`-stage failure requires `HmrRebuild` so the link/codegen
/// stage runs again.
///
/// See `meta/design/dev-engine.md` — Design principles §3, §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorStage {
  /// Failed in `plugin_driver.watch_change` or `generate_hmr_updates`.
  /// `watch_change` is grouped here because the next `Hmr` task re-runs
  /// the hook, which is sufficient to retry it.
  Hmr,
  /// Failed in `rebuild()` (incremental link/codegen). The bundle output
  /// is now stale w.r.t. the source; only a fresh rebuild can recover.
  Rebuild,
}
