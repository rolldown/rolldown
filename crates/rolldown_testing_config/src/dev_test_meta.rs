use rolldown_dev_common::types::DevOptions;
use schemars::JsonSchema;
use serde::Deserialize;

fn default_true() -> bool {
  true
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevTestMeta {
  #[serde(default)]
  pub config: Option<DevOptions>,
  #[serde(default)]
  /// If `true`, the test will call `ensure_latest_build_output()` after each HMR step to wait for async builds.
  /// This allows capturing all build outputs triggered by each step.
  /// Default is `false` for backwards compatibility and performance.
  pub ensure_latest_build_output_for_each_step: bool,
  #[serde(default = "default_true")]
  /// After each HMR step, run a fresh full build of the current file state
  /// and assert the incremental scan state matches it. Skipped automatically
  /// for steps whose state does not build and for steps whose incremental
  /// build failed (a failed scan is reverted, so the state intentionally
  /// stays at the last good build). Default is `true`.
  pub check_state_parity: bool,
}

impl Default for DevTestMeta {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
