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

/// A named-only import needs no interop at all, so its module's format must not decide the mode of
/// the wrapper a *default* import in the same chunk gets. Deconflicting already filters on
/// `specifier_needs_interop`; the renderers have to agree, or the chunk is planned single-mode and
/// then rendered in the other mode.
#[tokio::test(flavor = "multi_thread")]
async fn node_mode_named_only_import_does_not_flip_a_non_node_default_import() {
  let output = bundle_entries(vec![InputItem {
    name: Some("same-chunk-mixed-entry".to_string()),
    import: "./same-chunk-mixed-entry.js".to_string(),
  }])
  .await;
  let entry = &output["same-chunk-mixed-entry.js"];

  assert_eq!(
    entry.matches("__toESM(").count(),
    1,
    "only the default import needs interop, so exactly one wrapper is expected:\n{entry}"
  );
  assert!(
    !entry.contains(", 1);"),
    "the only interop observer is a non-Node default import, so the wrapper must not use Node \
     mode; the Node-mode importer here is named-only and reads the CommonJS object directly:\n\
     {entry}"
  );
}

/// One consumer reaching the same external through both a `.mjs` and a `.js` shim. Linking
/// collapses both onto the external's namespace symbol, so the two references are literally the
/// same canonical symbol and cannot render as different bindings.
///
/// Both flags therefore get recorded while only a non-ESM module survives to read them. Planning a
/// mixed-mode pair here would emit a node binding nothing reads, and DCE keeps the discarded
/// `__toESM(mod, 1)` call — an eager property walk, plus Proxy traps, for a dropped value. One
/// wrapper is emitted instead, in the mode matching the module that actually reads it.
///
/// The provenance mismatch itself is not fixed and is not fixable here: the `.mjs`-routed reference
/// reads the non-Node wrapper. That needs per-reference provenance in the finalizer, and is only
/// observable for an external declaring `__esModule` *and* owning a `default`.
#[tokio::test(flavor = "multi_thread")]
async fn dual_provenance_through_shims_of_different_formats() {
  let output = bundle_entries(vec![InputItem {
    name: Some("dual-provenance-entry".to_string()),
    import: "./dual-provenance-entry.js".to_string(),
  }])
  .await;
  let entry = &output["dual-provenance-entry.js"];

  assert_eq!(
    entry.matches("__toESM(").count(),
    1,
    "only the surviving non-ESM consumer reads the external, so exactly one wrapper is expected \
     — a second one would be dead on arrival and leave a discarded call behind:\n{entry}"
  );
  assert!(
    !entry.contains(", 1)"),
    "the only module reading the wrapper is non-ESM, so the wrapper must not use Node mode:\n\
     {entry}"
  );
}
