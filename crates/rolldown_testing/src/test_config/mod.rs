use std::path::Path;

use schemars::JsonSchema;
use serde::Deserialize;
pub mod input_options;
pub mod output_options;

#[macro_export]
macro_rules! impl_serde_default {
  ($name:ident) => {
    impl Default for $name {
      fn default() -> Self {
        serde_json::from_str("{}").expect("Failed to parse default config")
      }
    }
  };
}

fn true_by_default() -> bool {
  true
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TestConfig {
  #[serde(default)]
  pub input: input_options::InputOptions,
  #[serde(default)]
  pub output: output_options::OutputOptions,
  #[serde(default = "true_by_default")]
  /// If `false`, the compiled artifacts won't be executed.
  pub expect_executed: bool,
  #[serde(default)]
  /// If `true`, the fixture are expected to fail to compile/build.
  pub expect_error: bool,
  #[serde(default, rename = "_comment")]
  /// An workaround for writing comments in JSON.
  pub _comment: String,
  #[serde(default)]
  /// If `true`, the fixture output stats will be snapshot.
  pub snapshot_output_stats: bool,
  #[serde(default)]
  /// If `true`, the sourcemap visualizer will be snapshot.
  pub sourcemap: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedError {
  pub code: String,
  pub message: String,
}

impl TestConfig {
  pub fn from_config_path(filepath: &Path) -> Self {
    let config_str = std::fs::read_to_string(filepath).expect("Failed to read test config file");
    let test_config: Self =
      serde_json::from_str(&config_str).expect("Failed to parse test config file");
    test_config
  }
}
