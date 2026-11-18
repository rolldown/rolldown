use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, EmittedChunk, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest, OutputFormat,
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
    for id in ["./seg_a.js", "./seg_b.js"] {
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
async fn allow_extension_reexport_name_collision() {
  let options = BundlerOptions {
    input: Some(vec![InputItem { name: Some("entry".into()), import: "./entry.js".into() }]),
    preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new("seg_|helper\\.js$").unwrap())),
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
          description: Some("Cjs".to_string()),
          options: BundlerOptions { format: Some(OutputFormat::Cjs), ..options.clone() },
          snapshot: None,
          config_name: Some("Cjs".to_string()),
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("Esm".to_string()),
          options: BundlerOptions { format: Some(OutputFormat::Esm), ..options },
          snapshot: None,
          config_name: Some("Esm".to_string()),
          expect_execution_failure: None,
        },
      ],
      vec![Plugin::new_shared(EmitChunkPlugin)],
    )
    .await;
}
