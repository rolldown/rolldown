use std::path::PathBuf;

// use std::path::PathBuf;
use rolldown_testing::TestConfig;
use schemars::schema_for;

fn main() {
  let schema = schema_for!(TestConfig);
  let scheme_path =
    PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").expect("Should have CARGO_MANIFEST_DIR"))
      .join("test.config.scheme.json");
  std::fs::write(scheme_path, serde_json::to_string_pretty(&schema).expect("Should be valid json"))
    .expect("Should write successfully");
}
