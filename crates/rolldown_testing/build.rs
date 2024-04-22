use schemars::schema_for;
use serde_json::to_string_pretty;
use std::fs;
use std::path::PathBuf;

use rolldown_testing_config::TestConfig;

fn main() {
  let schema = schema_for!(TestConfig);
  let scheme_path =
    PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").expect("Should have CARGO_MANIFEST_DIR"))
      .join("_test.scheme.json");

  fs::write(scheme_path, to_string_pretty(&schema).expect("Should be valid JSON"))
    .expect("Failed to write schema");
}
