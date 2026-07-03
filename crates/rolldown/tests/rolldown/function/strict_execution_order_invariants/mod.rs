use std::collections::BTreeMap;

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest, Output,
};
use rolldown_utils::js_regex::HybridRegex;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/strict_execution_order_invariants");

async fn bundle_fixture(
  fixture_dir: &str,
  inputs: Vec<InputItem>,
  strict_execution_order: bool,
) -> BTreeMap<String, String> {
  bundle_fixture_with_options(
    fixture_dir,
    inputs,
    strict_execution_order,
    BundlerOptions::default(),
  )
  .await
}

async fn bundle_fixture_with_options(
  fixture_dir: &str,
  inputs: Vec<InputItem>,
  strict_execution_order: bool,
  mut options: BundlerOptions,
) -> BTreeMap<String, String> {
  options.format.get_or_insert(OutputFormat::Esm);
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(inputs),
    cwd: Some(fixture_dir.into()),
    entry_filenames: Some("[name].js".to_string().into()),
    chunk_filenames: Some("chunks/[name].js".to_string().into()),
    strict_execution_order: Some(strict_execution_order),
    ..options
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

async fn bundle(strict_execution_order: bool) -> BTreeMap<String, String> {
  bundle_fixture(
    FIXTURE_ROOT,
    vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
    strict_execution_order,
  )
  .await
}

#[tokio::test(flavor = "multi_thread")]
async fn strict_off_interop_esm_wrapper_keeps_legacy_shape() {
  let output = bundle_fixture_with_options(
    &format!("{FIXTURE_ROOT}/strict_off_interop_esm_wrapper"),
    vec![InputItem { name: Some("entry".to_string()), import: "./entry.js".to_string() }],
    false,
    BundlerOptions { format: Some(OutputFormat::Cjs), ..Default::default() },
  )
  .await;

  let entry_chunk = output.get("entry.js").expect("entry chunk should be emitted");
  assert!(
    entry_chunk.contains("const ns = (__esmMin((() => {"),
    "strict-off require-of-ESM should keep the historical inline init expression:\n{entry_chunk}",
  );
  assert!(
    !entry_chunk.contains("function init_esm()"),
    "strict-off interop wrappers must not use order-wrapper hoisting:\n{entry_chunk}",
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn strict_execution_order_does_not_change_hazard_free_output() {
  let default_output = bundle(false).await;
  let strict_output = bundle(true).await;

  assert_eq!(strict_output, default_output);
}

#[tokio::test(flavor = "multi_thread")]
async fn dynamic_entry_does_not_static_import_side_effectful_runtime_host() {
  let output = bundle_fixture(
    &format!("{FIXTURE_ROOT}/runtime_inert"),
    vec![
      InputItem { name: Some("e0".to_string()), import: "./e0.js".to_string() },
      InputItem { name: Some("e2".to_string()), import: "./e2.js".to_string() },
    ],
    true,
  )
  .await;

  let e2_chunk =
    output.values().find(|code| code.contains("reg.dyn1")).expect("e2 body should be emitted");

  assert!(
    !e2_chunk.lines().any(|line| line.starts_with("import ") && line.contains("m18")),
    "the e2 chunk must not statically import the side-effectful dynamic target:\n{e2_chunk}",
  );
  assert!(
    output.values().any(|code| code.contains("sfx-m18-0")),
    "fixture should still emit the side-effectful dynamic target",
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn wrapped_dynamic_entry_keeps_facade_after_manual_chunk_merge() {
  let output = bundle_fixture_with_options(
    &format!("{FIXTURE_ROOT}/m4_dynamic_facade_race"),
    vec![
      InputItem { name: Some("a".to_string()), import: "./a.js".to_string() },
      InputItem { name: Some("b".to_string()), import: "./b.js".to_string() },
    ],
    true,
    BundlerOptions {
      code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
        groups: Some(vec![MatchGroup {
          name: MatchGroupName::Static("dyn".to_string()),
          test: Some(MatchGroupTest::Regex(
            HybridRegex::new("target|observer").expect("regex should be valid"),
          )),
          ..Default::default()
        }]),
        ..Default::default()
      })),
      ..Default::default()
    },
  )
  .await;

  let a_chunk =
    output.values().find(|code| code.contains("a done")).expect("a entry should be emitted");
  assert!(
    a_chunk.contains("import(\"./chunks/target.js\")"),
    "a should import the restored dynamic facade directly:\n{a_chunk}",
  );
  assert!(
    !a_chunk.contains(".then((n) =>"),
    "a must not call the wrapped dynamic entry through a shared-chunk .then trigger:\n{a_chunk}",
  );

  let target_facade = output
    .get("chunks/target.js")
    .expect("wrapped dynamic target should keep an empty facade chunk");
  assert!(
    target_facade.contains("init_target();"),
    "the restored facade should trigger target initialization:\n{target_facade}",
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn restored_dynamic_facade_keeps_dependency_chunk_inert() {
  let output = bundle_fixture_with_options(
    &format!("{FIXTURE_ROOT}/m4_dynamic_facade_dependency"),
    vec![InputItem { name: Some("e0".to_string()), import: "./e0.js".to_string() }],
    true,
    BundlerOptions {
      code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
        groups: Some(vec![MatchGroup {
          name: MatchGroupName::Static("gb".to_string()),
          test: Some(MatchGroupTest::Regex(
            HybridRegex::new(r"[\\/](m29|m13)\.js$").expect("regex should be valid"),
          )),
          min_size: Some(0.0),
          ..Default::default()
        }]),
        min_size: Some(0.0),
        include_dependencies_recursively: Some(false),
        ..Default::default()
      })),
      ..Default::default()
    },
  )
  .await;

  let m31_chunk = output.get("chunks/m31.js").expect("m31 should be split into its own chunk");
  assert!(
    !m31_chunk.contains("init_m31();"),
    "loading the dependency chunk must not execute the dynamic entry dependency:\n{m31_chunk}",
  );

  let m29_facade = output.get("chunks/m29.js").expect("dynamic entry should keep a facade chunk");
  assert!(
    m29_facade.contains("init_m29();"),
    "the dynamic entry facade should trigger its wrapper only when imported:\n{m29_facade}",
  );
}
