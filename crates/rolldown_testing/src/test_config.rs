use std::fs;

use jsonschema::{Draft, JSONSchema};
use std::sync::LazyLock;

pub use rolldown_testing_config::{TestConfig, TestMeta};

use crate::workspace;

static COMPILED_SCHEMA: LazyLock<JSONSchema> = LazyLock::new(|| {
  let schema_path = workspace::crate_dir("rolldown_testing").join("_config.schema.json");

  let schema_str = fs::read_to_string(&schema_path)
    .unwrap_or_else(|e| panic!("Failed to read schema file {schema_path:?}. Got {e:?}"));

  let schema_json: serde_json::Value = serde_json::from_str(&schema_str).unwrap_or_else(|e| {
    panic!("Failed to parse test config file ${schema_path:?} in json. Got {e:?}")
  });

  JSONSchema::options()
    .with_draft(Draft::Draft7)
    .compile(&schema_json)
    .unwrap_or_else(|e| panic!("Failed to compile {schema_path:?} to json schema. Got {e:?}"))
});

pub fn read_test_config(config_path: &std::path::Path) -> TestConfig {
  let mut config_str = fs::read_to_string(config_path)
    .unwrap_or_else(|e| panic!("Failed to read config file in {config_path:?}. Got {e:?}"));

  json_strip_comments::strip(&mut config_str)
    .unwrap_or_else(|e| panic!("Failed to strip comments of {config_path:?}. Got {e:?}"));

  let config_json: serde_json::Value =
    serde_json::from_str(&config_str).expect("Failed to parse test config file");

  let result = COMPILED_SCHEMA.validate(&config_json);

  if let Err(errors) = result {
    let mut msg = String::new();
    for error in errors {
      msg.push_str(&format!("Validation error: {} in {}\n", error, error.instance_path));
    }
    panic!("Failed to validate test config {config_path:?}. Got {msg}");
  };

  drop(result);

  serde_json::from_value(config_json).expect("Failed to parse test config file")
}
