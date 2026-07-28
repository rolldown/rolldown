use std::collections::BTreeMap;

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat};
use rolldown_common::Output;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/external_interop_invariants");

async fn bundle_entries(input: Vec<InputItem>) -> BTreeMap<String, String> {
  let mut bundler = Bundler::new(BundlerOptions {
    cwd: Some(FIXTURE_ROOT.into()),
    input: Some(input),
    external: Some(vec!["node:https".to_string()].into()),
    format: Some(OutputFormat::Cjs),
    entry_filenames: Some("[name].js".to_string().into()),
    ..Default::default()
  })
  .expect("failed to create bundler");

  bundler
    .generate()
    .await
    .expect("build should succeed")
    .assets
    .into_iter()
    .filter_map(|output| match output {
      Output::Chunk(chunk) => Some((chunk.filename.to_string(), chunk.code.clone())),
      Output::Asset(_) => None,
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_default_observer_does_not_wrap_a_named_only_entry_chunk() {
  let output = bundle_entries(vec![
    InputItem { name: Some("default-user".to_string()), import: "./default-user.js".to_string() },
    InputItem { name: Some("named-user".to_string()), import: "./named-user.js".to_string() },
  ])
  .await;
  let default_entry = &output["default-user.js"];
  let named_entry = &output["named-user.js"];

  assert!(
    default_entry.contains("__toESM("),
    "the default-import entry must exercise external interop:\n{default_entry}"
  );
  assert!(
    named_entry.contains("require(\"node:https\")"),
    "the named-only entry must load the external directly:\n{named_entry}"
  );
  assert!(
    !named_entry.contains("__toESM("),
    "interop from a live observer in another chunk must not wrap this named-only external; \
     __toESM eagerly enumerates CommonJS exports and can trigger observable Proxy traps:\n\
     {named_entry}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_node_and_non_node_observers_keep_modes_chunk_local() {
  let output = bundle_entries(vec![
    InputItem {
      name: Some("node-mode-user".to_string()),
      import: "./node-mode-user.mjs".to_string(),
    },
    InputItem {
      name: Some("non-node-mode-user".to_string()),
      import: "./non-node-mode-user.js".to_string(),
    },
  ])
  .await;
  let node_entry = &output["node-mode-user.js"];
  let non_node_entry = &output["non-node-mode-user.js"];

  assert_eq!(
    node_entry.matches("__toESM(").count(),
    1,
    "the Node ESM observer must add exactly one interop wrapper:\n{node_entry}"
  );
  assert!(
    node_entry.contains(", 1);"),
    "the Node ESM observer's wrapper must use Node mode:\n{node_entry}"
  );

  assert_eq!(
    non_node_entry.matches("__toESM(").count(),
    1,
    "the non-Node ESM observer must add exactly one interop wrapper:\n{non_node_entry}"
  );
  assert!(
    !non_node_entry.contains(", 1);"),
    "the non-Node ESM observer's wrapper must not use Node mode:\n{non_node_entry}"
  );
}
