use std::{borrow::Cow, sync::Arc};

use anyhow::Ok;
use rolldown::{AssetFilenamesOutputOption, BundlerOptions, InputItem};
use rolldown_common::EmittedAsset;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

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
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_error: false, ..Default::default() }, abs_file_dir!())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        asset_filenames: Some(AssetFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:23].js".into(),
        )),
        ..Default::default()
      },
      vec![Arc::new(TestPlugin)],
    )
    .await;
}
