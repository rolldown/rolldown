use std::fmt::Write as _;
use std::fs;
use std::sync::LazyLock;

use jsonschema::{Draft, Validator};

pub use rolldown_testing_config::{TestConfig, TestMeta};

use rolldown_workspace::crate_dir;

static COMPILED_SCHEMA: LazyLock<Validator> = LazyLock::new(|| {
  let schema_path = crate_dir("rolldown_testing").join("_config.schema.json");

  let schema_str = fs::read_to_string(&schema_path)
    .unwrap_or_else(|e| panic!("Failed to read schema file {schema_path:?}. Got {e:?}"));

  let schema_json: serde_json::Value = serde_json::from_str(&schema_str).unwrap_or_else(|e| {
    panic!("Failed to parse test config file ${schema_path:?} in json. Got {e:?}")
  });

  Validator::options()
    .with_draft(Draft::Draft7)
    .build(&schema_json)
    .unwrap_or_else(|e| panic!("Failed to compile {schema_path:?} to json schema. Got {e:?}"))
});

pub fn read_test_config(config_path: &std::path::Path) -> TestConfig {
  let mut config_str = fs::read_to_string(config_path)
    .unwrap_or_else(|e| panic!("Failed to read config file in {config_path:?}. Got {e:?}"));

  json_strip_comments::strip(&mut config_str)
    .unwrap_or_else(|e| panic!("Failed to strip comments of {config_path:?}. Got {e:?}"));

  let config_json: serde_json::Value =
    serde_json::from_str(&config_str).expect("Failed to parse test config file");

  let errors = COMPILED_SCHEMA.iter_errors(&config_json);
  let mut msg = String::new();
  for error in errors {
    writeln!(msg, "Validation error: {} in {}", error, error.instance_path).unwrap();
  }
  assert!(msg.is_empty(), "Failed to validate test config {config_path:?}. Got {msg}");

  serde_json::from_value(config_json).expect("Failed to parse test config file")
}
