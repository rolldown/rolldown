use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, Plugin, SharedPluginContext,
};
use rolldown_sourcemap::{MissingSourceMap, SourceMapOrMissing};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[derive(Debug)]
struct SourcemapBroken;

impl Plugin for SourcemapBroken {
  fn name(&self) -> Cow<'static, str> {
    "sourcemap-broken-render-chunk".into()
  }

  async fn render_chunk(
    &self,
    _ctx: &SharedPluginContext,
    _args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    Ok(Some(HookRenderChunkOutput {
      code: String::new(),
      map: Some(SourceMapOrMissing::Missing(MissingSourceMap {
        plugin_name: Some(self.name().into()),
      })),
    }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_warn_if_hook_render_chunk_map_is_undefined() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(SourcemapBroken)],
    )
    .await;
}
