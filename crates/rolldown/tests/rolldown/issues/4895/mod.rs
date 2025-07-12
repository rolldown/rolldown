use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, PreserveEntrySignatures};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct Test;

impl Plugin for Test {
  fn name(&self) -> Cow<'static, str> {
    "test".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    ctx
      .emit_chunk(EmittedChunk {
        id: "./strict/main.js".into(),
        name: Some("strict".into()),
        preserve_entry_signatures: Some(PreserveEntrySignatures::Strict),
        ..Default::default()
      })
      .await?;
    ctx
      .emit_chunk(EmittedChunk {
        id: "./not-specified/main.js".into(),
        name: Some("not-specified".into()),
        ..Default::default()
      })
      .await?;
    ctx
      .emit_chunk(EmittedChunk {
        id: "./allow-extension/main.js".into(),
        name: Some("allow-extension".into()),
        preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
        ..Default::default()
      })
      .await?;
    ctx
      .emit_chunk(EmittedChunk {
        id: "./false/main.js".into(),
        name: Some("false".into()),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        ..Default::default()
      })
      .await?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_rewrite_dynamic_imports_that_import_external_modules() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(BundlerOptions { input: None, ..Default::default() }, vec![Arc::new(Test)])
    .await;
}
