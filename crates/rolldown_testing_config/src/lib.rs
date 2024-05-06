use jsonschema::JSONSchema;
use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools, clippy::pub_underscore_fields)]
pub struct TestConfig {
  #[serde(default)]
  pub config: rolldown_common::BundlerOptions,
  #[serde(default = "true_by_default")]
  /// If `false`, the compiled artifacts won't be executed.
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
}

fn true_by_default() -> bool {
  true
}

static COMPILED_SCHEMA: OnceCell<Mutex<JSONSchema>> = OnceCell::new();

impl TestConfig {
  pub fn from_config_path(filepath: &Path, schemapath: &Path) -> Self {
    let schema_str = fs::read_to_string(schemapath)
      .unwrap_or_else(|e| panic!("Failed to read schema file: {e:?}"));

    let schema_json: Value =
      serde_json::from_str(&schema_str).expect("Failed to parse schema JSON");

    let compiled_schema = JSONSchema::compile(&schema_json).expect("Failed to Compile Json Schema");

    let _ = COMPILED_SCHEMA.set(Mutex::new(compiled_schema));

    // Read the config file
    let config_str = fs::read_to_string(filepath).expect("Failed to read test config file");

    // Parse the config JSON
    let config_json: Value =
      serde_json::from_str(&config_str).expect("Failed to parse config JSON");

    // Validate against the schema and create file
    if COMPILED_SCHEMA.get().unwrap().lock().unwrap().validate(&config_json).is_ok() {
      fs::read_to_string(filepath).expect("Failed to read test config file");
      serde_json::from_str(&config_str).expect("Failed to parse test config file")
    } else {
      panic!("Validation failed for config JSON")
    }
  }
}
