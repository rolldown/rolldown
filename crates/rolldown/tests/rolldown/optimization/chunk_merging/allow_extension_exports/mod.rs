use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::{
  EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_plugin::{Plugin, PluginContext, RegisterHook};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

/// Test that emitted chunks with AllowExtension preserve entry signatures
/// can be properly merged with other chunks during chunk merging optimization.
#[derive(Debug)]
struct EmitChunkPlugin;

#[RegisterHook]
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
    ctx
      .emit_chunk(EmittedChunk {
        id: "./lib1.js".to_string(),
        preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
        ..Default::default()
      })
      .await?;
    ctx
      .emit_chunk(EmittedChunk {
        id: "./lib2.js".to_string(),
        preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
        ..Default::default()
      })
      .await?;
    ctx
      .emit_chunk(EmittedChunk {
        id: "./lib3.js".to_string(),
        preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
        ..Default::default()
      })
      .await?;
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_exports() {
  // This test verifies that when lib files are:
  // 1. Dynamically imported from index.js (lib3.js)
  // 2. Emitted via this.emitFile with AllowExtension preserve entry signatures
  // 3. Captured by manual_code_splitting groups
  // The chunk merging optimization works correctly.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        manual_code_splitting: Some(ManualCodeSplittingOptions {
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("libs".to_string()),
            test: Some(MatchGroupTest::Regex(HybridRegex::new("lib").unwrap())),
            ..Default::default()
          }]),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Arc::new(EmitChunkPlugin)],
    )
    .await;
}
