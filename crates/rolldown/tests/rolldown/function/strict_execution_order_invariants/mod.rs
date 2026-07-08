use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest, Output,
};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_utils::js_regex::HybridRegex;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/strict_execution_order_invariants");

#[derive(Debug)]
struct EmitTarget;

impl Plugin for EmitTarget {
  fn name(&self) -> Cow<'static, str> {
    "emit-target".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    ctx.emit_chunk(EmittedChunk {
      name: Some("target".into()),
      id: "./target.js".to_string(),
      preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
      ..Default::default()
    })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

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

async fn bundle_emitted_target(fixture_name: &str) -> BTreeMap<String, String> {
  let fixture_dir = format!("{FIXTURE_ROOT}/{fixture_name}");
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("host".to_string()),
        import: "./host.js".to_string(),
      }]),
      cwd: Some(fixture_dir.into()),
      format: Some(OutputFormat::Esm),
      entry_filenames: Some("[name].js".to_string().into()),
      chunk_filenames: Some("chunks/[name].js".to_string().into()),
      strict_execution_order: Some(true),
      code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
        groups: Some(vec![MatchGroup {
          name: MatchGroupName::Static("group".to_string()),
          test: Some(MatchGroupTest::Regex(
            HybridRegex::new(r"(target|dep-a|dep-b)\.").expect("regex should be valid"),
          )),
          ..Default::default()
        }]),
        ..Default::default()
      })),
      ..Default::default()
    },
    vec![Arc::new(EmitTarget)],
  )
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
async fn order_wrapper_entry_uses_explicit_prologue() {
  let fixture_dir = format!("{FIXTURE_ROOT}/../experimental/strict_execution_order/issue_4782");
  let output = bundle_fixture(
    &fixture_dir,
    vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
    true,
  )
  .await;

  // `main.js` is a plain ESM entry, so its immutable interop wrap kind is `None`.
  let entry_chunk = output.get("main.js").expect("main entry facade should be emitted");
  assert!(
    entry_chunk.contains("init_main();"),
    "the order-wrapped entry facade must explicitly trigger its init target:\n{entry_chunk}",
  );
  assert!(
    output.values().any(|code| code.contains("function init_main()")),
    "the entry implementation should contain the hoisted execution-order wrapper",
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn order_runtime_helpers_include_dependency_closure() {
  let fixture_dir = format!("{FIXTURE_ROOT}/../experimental/strict_execution_order/issue_4782");
  let output = bundle_fixture_with_options(
    &fixture_dir,
    vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
    true,
    BundlerOptions { profiler_names: Some(true), ..Default::default() },
  )
  .await;

  let runtime_chunk = output
    .values()
    .find(|code| code.contains("var __esm ="))
    .expect("profiler wrapper should emit the named ESM runtime helper");
  assert!(
    runtime_chunk.contains("var __getOwnPropNames ="),
    "`__esm` dependencies must be retained by the order runtime closure:\n{runtime_chunk}",
  );
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

#[tokio::test(flavor = "multi_thread")]
async fn emitted_dynamic_entry_keeps_order_wrapper_facade() {
  let output = bundle_emitted_target("emitted_dynamic_entry").await;
  let host = output.get("host.js").expect("host entry should be emitted");
  assert!(
    host.contains("import(\"./chunks/target.js\")"),
    "the dynamic import should target the restored facade:\n{host}",
  );
  let target = output.get("chunks/target.js").expect("target facade should be restored");
  assert!(
    target.contains("init_target();"),
    "target facade should trigger initialization:\n{target}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_keeps_order_wrapper_facade() {
  let output = bundle_emitted_target("emitted_entry").await;
  let target = output.get("chunks/target.js").expect("target facade should be restored");
  assert!(
    target.contains("init_target();"),
    "target facade should trigger initialization:\n{target}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn late_order_wrapping_revalidates_output_file() {
  let fixture_dir = format!("{FIXTURE_ROOT}/../experimental/strict_execution_order/issue_4782");
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("main".to_string()),
      import: "./main.js".to_string(),
    }]),
    cwd: Some(fixture_dir.into()),
    file: Some("bundle.js".to_string()),
    format: Some(OutputFormat::Esm),
    strict_execution_order: Some(true),
    ..Default::default()
  })
  .expect("failed to create bundler");

  let Err(error) = bundler.generate().await else {
    panic!("output.file must reject the final multi-chunk graph after order wrapping");
  };
  let message = error.to_string();
  assert!(
    message.contains("When building multiple chunks") && message.contains("output.file"),
    "unexpected diagnostic: {message}"
  );
}
