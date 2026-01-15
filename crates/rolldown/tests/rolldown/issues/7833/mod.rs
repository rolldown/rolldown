use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Test that when a module is both dynamically imported AND emitted via this.emitFile,
/// only one entry chunk is created (emitted entry takes priority).
/// See: https://github.com/rolldown/rolldown/issues/7833
#[derive(Debug)]
struct Test;

impl Plugin for Test {
  fn name(&self) -> Cow<'static, str> {
    "test".into()
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    // Emit chunk for dynamically imported module
    if args.id.ends_with("imp.js") {
      ctx
        .emit_chunk(EmittedChunk {
          id: args.id.to_string(),
          preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
          ..Default::default()
        })
        .await?;
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn deduplicate_emit_file_and_dynamic_import() {
  // This test verifies that when imp.js is both:
  // 1. Dynamically imported from main.js
  // 2. Emitted via this.emitFile in transform hook
  // Only ONE chunk for imp.js is created (not two duplicate chunks)
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        ..Default::default()
      },
      vec![Arc::new(Test)],
    )
    .await;
}
