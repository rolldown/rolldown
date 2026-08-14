use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest,
};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

/// Test that emitted chunks with AllowExtension preserve entry signatures
/// can be properly merged with other chunks during chunk merging optimization.
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
    // Emit all library files as entry chunks with AllowExtension
    ctx.emit_chunk(EmittedChunk {
      id: "./lib1.js".to_string(),
      preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
      ..Default::default()
    })?;
    ctx.emit_chunk(EmittedChunk {
      id: "./lib2.js".to_string(),
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
async fn allow_extension_merge_aliased_then_locals() {
  // Two emitted AllowExtension entries each declare a local `then` but export it under a
  // different alias, so neither exports the *name* `then`. With internal export names kept,
  // both symbols must go through the resolver — handing `then` to either would put a
  // callable `then` on the merged chunk's namespace (or, to both, emit a duplicate export).
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        minify_internal_exports: Some(false),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("libs".to_string()),
            test: Some(MatchGroupTest::Regex(HybridRegex::new("lib").unwrap())),
            ..Default::default()
          }]),
          ..Default::default()
        })),
        ..Default::default()
      },
      vec![Arc::new(EmitChunkPlugin)],
    )
    .await;
}
