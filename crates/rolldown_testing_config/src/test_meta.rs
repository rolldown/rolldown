use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
  dev_test_meta::DevTestMeta, extended_tests::ExtendedTests, plugin_test_meta::PluginTestMeta,
  utils::true_by_default,
};

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[expect(clippy::struct_excessive_bools, clippy::pub_underscore_fields)]
pub struct TestMeta {
  #[serde(default = "true_by_default")]
  /// If `false`, the compiled artifacts won't be executed, but `_test.mjs` will be still executed if exists.
  pub expect_executed: bool,
  #[serde(default)]
  /// If `true`, the fixture are expected to fail to compile/build.
  pub expect_error: bool,
  #[serde(default, rename = "_comment")]
  /// A workaround for writing comments in JSON.
  pub _comment: String,
  #[serde(default)]
  /// If `true`, the fixture output stats will be snapshot.
  pub snapshot_output_stats: bool,
  #[serde(default)]
  /// If `true`, the sourcemap visualizer will be snapshot.
  pub visualize_sourcemap: bool,
  #[serde(default)]
  /// If `true`, bytes source will be snapshot.
  pub snapshot_bytes: bool,
  #[serde(default = "true_by_default")]
  /// Default is `true`. If `false`, the runtime module will not be hidden.
  pub hidden_runtime_module: bool,
  /// If `true`, the `[hash]` pattern will be inserted in the `xxxxFilenames`.
  #[serde(default)]
  pub hash_in_filename: bool,
  /// If `true`, skip syntax validation of output chunks. Useful for tests that
  /// intentionally produce invalid output (e.g., testing invalid configuration warnings).
  #[serde(default)]
  pub skip_syntax_validation: bool,
  /// If `true`, the bundle will be called with `write()` instead of `generate()`.
  #[serde(default = "true_by_default")]
  pub write_to_disk: bool,
  /// Controls whether snapshots should be generated
  #[serde(default = "true_by_default")]
  pub snapshot: bool,
  #[serde(default)]
  pub extended_tests: ExtendedTests,
  #[serde(default)]
  /// Value will be injected into the `globalThis.__configName`
  pub config_name: Option<String>,
  #[serde(default)]
  /// Controls test behavior in the dev scenario
  pub dev: DevTestMeta,
  #[serde(default)]
  /// Plugin configurations to enable for this test fixture
  pub plugin: PluginTestMeta,
}

impl Default for TestMeta {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
