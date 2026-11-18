use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest,
};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
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
    for id in ["./seg_dynamic.js", "./seg_static.js"] {
      ctx.emit_chunk(EmittedChunk {
        id: id.to_string(),
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

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_merge_cross_chunk_reexport() {
  // Both emitted AllowExtension entries lose their facades to the `group` chunk, but the
  // bindings they re-export live in the `task.js` chunk. The group chunk must import them and
  // export them under the entries' names. `seg_dynamic.js` is also dynamically imported, so its
  // namespace object already imports `task`; `seg_static.js` has nothing else that imports
  // `helper` into the group (issue #11767).
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          include_dependencies_recursively: Some(false),
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("group".to_string()),
            test: Some(MatchGroupTest::Regex(HybridRegex::new("seg_").unwrap())),
            ..Default::default()
          }]),
          ..Default::default()
        })),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitChunkPlugin)],
    )
    .await;
}
