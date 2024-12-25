use std::fmt::Display;

use rolldown_common::{BundlerOptions, OutputExports, OutputFormat};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigVariant {
  pub format: Option<OutputFormat>,
  pub extend: Option<bool>,
  pub name: Option<String>,
  pub exports: Option<OutputExports>,
}

impl ConfigVariant {
  pub fn apply(&self, config: &rolldown_common::BundlerOptions) -> BundlerOptions {
    let mut config = config.clone();
    if let Some(format) = &self.format {
      config.format = Some(*format);
    }
    if let Some(exports) = &self.exports {
      config.exports = Some(*exports);
    }
    if let Some(extend) = &self.extend {
      config.extend = Some(*extend);
    }
    if let Some(name) = &self.name {
      config.name = Some(name.to_string());
    }
    config
  }
}

impl Display for ConfigVariant {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut fields = vec![];
    if let Some(format) = &self.format {
      fields.push(format!("format: {format:?}"));
    }
    if let Some(extend) = &self.extend {
      fields.push(format!("extend: {extend:?}"));
    }
    if let Some(name) = &self.name {
      fields.push(format!("name: {name:?}"));
    }
    if let Some(exports) = &self.exports {
      fields.push(format!("exports: {exports:?}"));
    }
    fields.sort();
    if fields.is_empty() {
      write!(f, "()")
    } else {
      write!(f, "({})", fields.join(", "))
    }
  }
}

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools, clippy::pub_underscore_fields)]
pub struct TestConfig {
  #[serde(default)]
  pub config: rolldown_common::BundlerOptions,
  #[serde(default)]
  // Each config variant will be extended into the main config and executed.
  pub config_variants: Vec<ConfigVariant>,
  #[serde(default, flatten)]
  pub meta: TestMeta,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools, clippy::pub_underscore_fields)]
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
  /// If `true`, the bundle will be called with `write()` instead of `generate()`.
  #[serde(default = "true_by_default")]
  pub write_to_disk: bool,
}

impl Default for TestMeta {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}

fn true_by_default() -> bool {
  true
}
