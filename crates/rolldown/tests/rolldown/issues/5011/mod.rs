use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

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
    if args.id.ends_with("after-preload-dynamic.js") {
      ctx
        .inner
        .emit_chunk(EmittedChunk {
          id: "./src/after-preload-dynamic.js".into(),
          preserve_entry_signatures: None,
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
async fn should_rewrite_dynamic_imports_that_import_external_modules() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        cwd: Some(cwd),
        input: Some(vec![InputItem {
          name: Some("entry".into()),
          import: "./src/entry.js".into(),
        }]),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        ..Default::default()
      },
      vec![Arc::new(Test)],
    )
    .await;
}
