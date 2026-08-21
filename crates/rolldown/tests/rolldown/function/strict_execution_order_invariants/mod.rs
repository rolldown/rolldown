use std::{
  borrow::Cow,
  collections::BTreeMap,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest, Output,
};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_utils::js_regex::HybridRegex;

const FIXTURE_ROOT: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/function/strict_execution_order_invariants");

struct WrittenBundle {
  assets: BTreeMap<String, String>,
  output_dir: PathBuf,
}

impl Drop for WrittenBundle {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.output_dir);
  }
}

#[derive(Debug)]
struct EmitTarget {
  id: &'static str,
  names: &'static [&'static str],
}

impl Plugin for EmitTarget {
  fn name(&self) -> Cow<'static, str> {
    "emit-target".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    for &name in self.names {
      ctx.emit_chunk(EmittedChunk {
        name: Some(name.into()),
        id: self.id.to_string(),
        preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
        ..Default::default()
      })?;
    }
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
  if strict_execution_order {
    // These invariants pin the on-demand mode unless a test opts into wrap-all explicitly.
    let mut experimental = options.experimental.take().unwrap_or_default();
    experimental.on_demand_wrapping.get_or_insert(true);
    options.experimental = Some(experimental);
  }
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

async fn bundle_emitted_target(
  fixture_name: &str,
  names: &'static [&'static str],
) -> WrittenBundle {
  let fixture_dir = format!("{FIXTURE_ROOT}/{fixture_name}");
  let output_dir = std::env::temp_dir().join(format!(
    "rolldown-strict-order-emitted-{}-{fixture_name}-{}",
    std::process::id(),
    names.join("-")
  ));
  let _ = std::fs::remove_dir_all(&output_dir);
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
      dir: Some(output_dir.to_string_lossy().into_owned()),
      strict_execution_order: Some(true),
      experimental: Some(rolldown_common::ExperimentalOptions {
        on_demand_wrapping: Some(true),
        ..Default::default()
      }),
      code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
        groups: Some(vec![MatchGroup {
          name: MatchGroupName::Static("group".to_string()),
          test: Some(MatchGroupTest::Regex(
            HybridRegex::new(r"[\\/](?:target|dep-a|dep-b)\.js$").expect("regex should be valid"),
          )),
          ..Default::default()
        }]),
        ..Default::default()
      })),
      ..Default::default()
    },
    vec![Arc::new(EmitTarget { id: "./target.js", names })],
  )
  .expect("failed to create bundler");

  let assets = bundler
    .write()
    .await
    .expect("build should succeed")
    .assets
    .into_iter()
    .filter_map(|output| match output {
      Output::Chunk(chunk) => Some((chunk.filename.to_string(), chunk.code.clone())),
      Output::Asset(_) => None,
    })
    .collect();
  std::fs::write(output_dir.join("package.json"), "{\"type\":\"module\"}\n")
    .expect("package marker should be written");

  WrittenBundle { assets, output_dir }
}

fn execute_written_bundle(output_dir: &Path, script: &str) {
  let output = Command::new("node")
    .current_dir(output_dir)
    .args(["--input-type=module", "--eval", script])
    .output()
    .expect("Node.js should execute the emitted bundle");
  assert!(
    output.status.success(),
    "emitted bundle execution failed\nstdout:\n{}\nstderr:\n{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr),
  );
}

fn assert_order_wrapper_facade(
  output: &BTreeMap<String, String>,
  facade_name: &str,
  init_name: &str,
) {
  let facade = output.get(facade_name).unwrap_or_else(|| panic!("{facade_name} should be emitted"));
  let definition = format!("function {init_name}()");
  let call = format!("{init_name}();");

  assert!(
    facade.lines().any(|line| line.starts_with("import ") && line.contains(init_name)),
    "{facade_name} must import {init_name}:\n{facade}",
  );
  assert!(facade.contains(&call), "{facade_name} must call {init_name}:\n{facade}");
  assert!(
    !facade.contains(&definition),
    "{facade_name} must remain separate from the wrapper implementation:\n{facade}",
  );

  let definition_count = output.values().filter(|code| code.contains(&definition)).count();
  assert_eq!(
    definition_count,
    1,
    "{init_name} must have exactly one implementation; emitted files were {:?}",
    output.keys().collect::<Vec<_>>(),
  );
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
async fn on_demand_wrapping_does_not_change_hazard_free_output() {
  let default_output = bundle(false).await;
  let strict_output = bundle(true).await;

  assert_eq!(
    strict_output, default_output,
    "on-demand wrapping must preserve byte-identical hazard-free output"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn wrap_all_mode_wraps_even_hazard_free_output() {
  let flag_off = bundle(false).await;
  let wrap_all = bundle_fixture_with_options(
    FIXTURE_ROOT,
    vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
    true,
    BundlerOptions {
      experimental: Some(rolldown_common::ExperimentalOptions {
        on_demand_wrapping: Some(false),
        ..Default::default()
      }),
      ..Default::default()
    },
  )
  .await;

  assert_ne!(flag_off, wrap_all, "wrap-all mode must wrap regardless of hazards");
  assert!(
    wrap_all.values().any(|code| code.contains("init_")),
    "wrap-all output should contain order wrappers",
  );
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
  let output = bundle_fixture_with_options(
    FIXTURE_ROOT,
    vec![InputItem { name: Some("main".to_string()), import: "./main.js".to_string() }],
    true,
    BundlerOptions {
      profiler_names: Some(true),
      experimental: Some(rolldown_common::ExperimentalOptions {
        on_demand_wrapping: Some(false),
        ..Default::default()
      }),
      ..Default::default()
    },
  )
  .await;

  let runtime_chunk = output
    .values()
    .find(|code| code.contains("var __esm ="))
    .expect("wrap-all profiler mode should emit the named ESM runtime helper");
  assert!(
    !runtime_chunk.contains("var __commonJS ="),
    "the fixture must not retain `__getOwnPropNames` through a CJS helper:\n{runtime_chunk}",
  );
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
async fn wrapped_dynamic_entry_uses_the_call_site_trigger_after_manual_chunk_merge() {
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
            HybridRegex::new(r"[\\/](?:target|observer)\.js$").expect("regex should be valid"),
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
  let host_import = "import(\"./chunks/dyn.js\")";
  assert!(
    a_chunk.contains(host_import),
    "a should import the chunk hosting the dynamic target:\n{a_chunk}",
  );
  let after_host_import =
    a_chunk.split_once(host_import).expect("the host import was checked above").1.trim_start();
  assert!(
    after_host_import.starts_with(".then("),
    "the import() rewrite must carry the target's trigger:\n{a_chunk}",
  );
  assert!(
    !output.keys().any(|name| name.contains("target.js")),
    "no facade file should be emitted for the dynamic target: {:?}",
    output.keys().collect::<Vec<_>>(),
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
      preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
      ..Default::default()
    },
  )
  .await;

  let m31_chunk = output.get("chunks/m31.js").expect("m31 should be split into its own chunk");
  assert!(
    m31_chunk.contains("function init_m31()"),
    "the dynamic dependency side effect must be enclosed by its wrapper:\n{m31_chunk}",
  );
  assert!(
    !m31_chunk.contains("init_m31();"),
    "loading the dependency chunk must not execute the dynamic entry dependency:\n{m31_chunk}",
  );

  assert_order_wrapper_facade(&output, "chunks/m29.js", "init_m29");
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_dynamic_entry_keeps_order_wrapper_facade() {
  let bundle = bundle_emitted_target("emitted_dynamic_entry", &["target"]).await;
  let host = bundle.assets.get("host.js").expect("host entry should be emitted");
  assert!(
    host.contains("import(\"./chunks/target.js\")"),
    "the dynamic import should target the restored facade:\n{host}",
  );
  assert_order_wrapper_facade(&bundle.assets, "chunks/target.js", "init_target");
  execute_written_bundle(
    &bundle.output_dir,
    r"
      const assert = (await import('node:assert')).default;
      globalThis.events = [];
      const { load } = await import('./host.js');
      assert.deepStrictEqual(globalThis.events, [], 'loading the host must not execute the emitted entry');
      await load();
      assert.deepStrictEqual(globalThis.events, ['dep-a', 'dep-b', 'target']);
    ",
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_keeps_order_wrapper_facade() {
  let bundle = bundle_emitted_target("emitted_entry", &["target"]).await;
  assert_order_wrapper_facade(&bundle.assets, "chunks/target.js", "init_target");
  execute_written_bundle(
    &bundle.output_dir,
    r"
      const assert = (await import('node:assert')).default;
      globalThis.events = [];
      await import('./host.js');
      assert.deepStrictEqual(globalThis.events, [], 'loading the host must not execute the emitted entry');
      await import('./chunks/target.js');
      assert.deepStrictEqual(globalThis.events, ['dep-a', 'dep-b', 'target']);
    ",
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicate_emitted_entries_keep_order_wrapper_facades() {
  let bundle = bundle_emitted_target("emitted_entry", &["target-a", "target-b"]).await;
  for name in ["target-a", "target-b"] {
    assert_order_wrapper_facade(&bundle.assets, &format!("chunks/{name}.js"), "init_target");
    execute_written_bundle(
      &bundle.output_dir,
      &format!(
        r"
          const assert = (await import('node:assert')).default;
          globalThis.events = [];
          await import('./host.js');
          assert.deepStrictEqual(globalThis.events, [], 'loading the host must not execute an emitted entry');
          await import('./chunks/{name}.js');
          assert.deepStrictEqual(globalThis.events, ['dep-a', 'dep-b', 'target']);
        ",
      ),
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn late_order_wrapping_revalidates_output_file() {
  // `output.file` is validated against the *final* chunk graph, so the shape that exercises it has
  // to render as one chunk after chunk optimization and gain a second chunk only during order
  // lowering.
  //
  // The `grp` group puts every module in one manual chunk, which leaves the user entry chunk empty
  // and folds the group back into it; the chunk emitted for the same entry module is likewise an
  // empty facade the optimizer merges into the group. That is one rendered chunk. Order lowering
  // then revives the emitted facade — an `emitFile` reference id has to resolve to a real file —
  // and only then is the graph multi-chunk. Flipping `strict_execution_order` off makes this build
  // succeed, which is what keeps the assertion below honest.
  //
  // `lib.js` pulls in a side-effectful `probe.js`, which keeps the entry's load closure
  // order-sensitive. Without that the wrapper is skippable — the entry would never be wrapped,
  // nothing would be restored, and the graph would stay single-chunk, testing nothing.
  let fixture_dir = format!("{FIXTURE_ROOT}/late_split_revalidates_output_file");
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(fixture_dir.into()),
      file: Some("bundle.js".to_string()),
      format: Some(OutputFormat::Esm),
      strict_execution_order: Some(true),
      code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
        groups: Some(vec![MatchGroup {
          name: MatchGroupName::Static("grp".to_string()),
          test: Some(MatchGroupTest::Regex(
            HybridRegex::new(r"[\\/](?:entry|lib|probe)\.js$").expect("regex should be valid"),
          )),
          ..Default::default()
        }]),
        ..Default::default()
      })),
      ..Default::default()
    },
    vec![Arc::new(EmitTarget { id: "./entry.js", names: &["emitted"] })],
  )
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

#[tokio::test(flavor = "multi_thread")]
async fn disabled_splitting_emitted_entry_routes_consumer_local_barrel() {
  // `codeSplitting: false` rejects multiple `input`s at option validation, but a plugin-emitted
  // chunk still adds a second entry whose per-entry placement consumes the pre-chunk probe's
  // consumer-local routes. The inert-plan carve-out in `apply_order_wraps` must therefore key on
  // the entry count rather than the option alone: with the entry-count clause dropped, this build
  // takes the no-lowering fast path while placement has already split the CJS leaves per entry,
  // and the emitted chunks `require()` each other in the #10515 startup cycle.
  for on_demand in [false, true] {
    let fixture_dir = format!("{FIXTURE_ROOT}/disabled_splitting_emitted_entry");
    let output_dir = std::env::temp_dir().join(format!(
      "rolldown-strict-order-disabled-emitted-{}-{}",
      std::process::id(),
      if on_demand { "on-demand" } else { "wrap-all" },
    ));
    let _ = std::fs::remove_dir_all(&output_dir);
    let mut bundler = Bundler::with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry-a".to_string()),
          import: "./entry-a.cjs".to_string(),
        }]),
        cwd: Some(fixture_dir.into()),
        format: Some(OutputFormat::Cjs),
        entry_filenames: Some("[name].js".to_string().into()),
        chunk_filenames: Some("chunks/[name].js".to_string().into()),
        dir: Some(output_dir.to_string_lossy().into_owned()),
        strict_execution_order: Some(true),
        code_splitting: Some(CodeSplittingMode::Bool(false)),
        experimental: Some(rolldown_common::ExperimentalOptions {
          on_demand_wrapping: Some(on_demand),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Arc::new(EmitTarget { id: "./entry-b.cjs", names: &["entry-b"] })],
    )
    .expect("failed to create bundler");

    let assets: BTreeMap<String, String> = bundler
      .write()
      .await
      .expect("build should succeed")
      .assets
      .into_iter()
      .filter_map(|output| match output {
        Output::Chunk(chunk) => Some((chunk.filename.to_string(), chunk.code.clone())),
        Output::Asset(_) => None,
      })
      .collect();
    let bundle = WrittenBundle { assets, output_dir };
    assert!(
      bundle.assets.contains_key("chunks/entry-b.js"),
      "emitted entry should be a separate chunk; emitted files were {:?}",
      bundle.assets.keys().collect::<Vec<_>>(),
    );
    execute_written_bundle(
      &bundle.output_dir,
      "import assert from 'node:assert';\n\
       const entryA = await import('./entry-a.js');\n\
       assert.strictEqual(entryA.a, 'a');\n\
       const entryB = await import('./chunks/entry-b.js');\n\
       assert.strictEqual(entryB.b, 'b');",
    );
  }
}
