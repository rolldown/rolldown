use schemars::schema_for;
use serde_json::to_string_pretty;
use std::fs;
use std::path::PathBuf;

use rolldown_testing_config::TestConfig;

fn main() {
  // If the definition of `TestConfig` changes, this build script will automatically re-run due to we rely on `rolldown_testing_config` in `Cargo.toml` already.
  // So we only add `build.rs` as the dependency to prevent unnecessary re-runs for every `cargo build`
  println!("cargo:rerun-if-changed=build.rs");
  let schema = schema_for!(TestConfig);
  let scheme_path =
    PathBuf::from(&std::env::var("CARGO_MANIFEST_DIR").expect("Should have CARGO_MANIFEST_DIR"))
      .join("_config.schema.json");

  fs::write(scheme_path, to_string_pretty(&schema).expect("Should be valid JSON"))
    .expect("Failed to write schema");
}
