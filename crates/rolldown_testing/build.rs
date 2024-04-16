use schemars::schema_for;
use serde_json::to_string_pretty;
use std::env;
use std::fs;
use std::path::PathBuf;

use rolldown_testing_config::TestConfig;

fn main() {
  println!("cargo:rerun-if-changed=src/test_config/mod.rs");
  println!("cargo:rerun-if-changed=../rolldown_common/src/inner_bundler_options/mod.rs");

  let schema = schema_for!(TestConfig);
  let out_dir = env::var("OUT_DIR").unwrap();
  let schema_output_path = PathBuf::from(&out_dir).join("_test.schema.json");

  fs::write(schema_output_path, to_string_pretty(&schema).expect("Should be valid JSON"))
    .expect("Failed to write schema");
}
