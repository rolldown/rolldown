use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem, OutputFormat, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest,
};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};
use rolldown_utils::js_regex::HybridRegex;

#[derive(Debug)]
struct EmitChunkPlugin;

impl Plugin for EmitChunkPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emit-chunk-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    ctx.emit_chunk(EmittedChunk {
      id: "./seg.js".to_string(),
      preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
      ..Default::default()
    })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_two_entries() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue #11767). The `a.js`
  // and `b.js` entries export `n` of `counter.cjs`. `main.js` loads them with `import()`, so
  // neither chunk can export an accessor. The facade re-exports `aval` from `a.js` and `n` from
  // `b.js`, so it imports both chunks, and the `a.js` chunk comes first. The facade reads `n`
  // through the public export `n` of the `b.js` chunk, because this chunk holds the namespace that
  // the facade reads `n` from. Thus the facade keeps its import of the `b.js` chunk. The `group`
  // chunk also imports the `b.js` chunk, so the test values do not show a lost import. The snapshot
  // shows the import.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("a".into()), import: "./a.js".into() },
      InputItem { name: Some("b".into()), import: "./b.js".into() },
      InputItem { name: Some("app2".into()), import: "./app2.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
      InputItem { name: Some("main".into()), import: "./main.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"seg\.js$").unwrap())),
        ..Default::default()
      }]),
      ..Default::default()
    })),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_multiple(
      vec![
        NamedBundlerOptions {
          description: None,
          options: options.clone(),
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("cjs".to_string()),
          options: BundlerOptions { format: Some(OutputFormat::Cjs), ..options },
          snapshot: None,
          config_name: Some("cjs".to_string()),
          expect_execution_failure: None,
        },
      ],
      vec![Plugin::new_shared(EmitChunkPlugin)],
    )
    .await;
}
