use std::{borrow::Cow, sync::Arc};

use rolldown::{AssetFilenamesOutputOption, BundlerOptions, InputItem};
use rolldown_common::EmittedAsset;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct TestPlugin;

impl Plugin for TestPlugin {
  fn name(&self) -> Cow<'static, str> {
    "test-plugin".into()
  }

  async fn render_chunk(
    &self,
    ctx: &PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    ctx.emit_file(
      EmittedAsset {
        file_name: None,
        original_file_name: None,
        name: Some("res.js".into()),
        source: args.code.clone().into(),
      },
      None,
      None,
    );
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn test() {
  manual_integration_test!()
    .build(TestMeta { expect_error: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "entry.js".to_string(),
        }]),
        asset_filenames: Some(AssetFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:23].js".into(),
        )),
        ..Default::default()
      },
      vec![Arc::new(TestPlugin)],
    )
    .await;
}
