use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedAsset;
use rolldown_plugin::{Plugin, PluginContext};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[derive(Debug)]
struct TestPlugin;

impl Plugin for TestPlugin {
  fn name(&self) -> Cow<'static, str> {
    "test-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.emit_file(
      EmittedAsset {
        file_name: None,
        original_file_name: None,
        name: Some("emitted.txt".into()),
        source: "emitted".to_string().into(),
      },
      None,
      None,
    );

    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn test() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_error: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("main".to_string()),
          import: "./main.js".to_string(),
        }]),
        cwd: Some(cwd),
        entry_filenames: Some(rolldown::ChunkFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:11]-[hash:22].js".into(),
        )),
        chunk_filenames: Some(rolldown::ChunkFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:11]-[hash:22].js".into(),
        )),
        css_entry_filenames: Some(rolldown::ChunkFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:11]-[hash:22].css".into(),
        )),
        css_chunk_filenames: Some(rolldown::ChunkFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:11]-[hash:22].css".into(),
        )),
        asset_filenames: Some(rolldown::AssetFilenamesOutputOption::String(
          "[name]-[hash]-[hash:1]-[hash:11]-[hash:22][extname]".into(),
        )),
        ..Default::default()
      },
      vec![Arc::new(TestPlugin)],
    )
    .await;
}
