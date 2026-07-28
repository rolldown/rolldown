use std::collections::BTreeMap;

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat};
use rolldown_common::Output;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/external_interop_invariants");

async fn bundle_entries() -> BTreeMap<String, String> {
  let mut bundler = Bundler::new(BundlerOptions {
    cwd: Some(FIXTURE_ROOT.into()),
    input: Some(vec![
      InputItem { name: Some("default-user".to_string()), import: "./default-user.js".to_string() },
      InputItem { name: Some("named-user".to_string()), import: "./named-user.js".to_string() },
    ]),
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
  let output = bundle_entries().await;
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
