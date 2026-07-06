use rolldown_dev_common::types::DevOptions;
use schemars::JsonSchema;
use serde::Deserialize;

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
  #[serde(default)]
  /// If `true`, HMR steps run against a client with an empty `executed_modules`
  /// set instead of the always-executed "rolldown-tests" client.
  pub unregistered_client: bool,
}

impl Default for DevTestMeta {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
