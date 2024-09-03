use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, Plugin, TransformPluginContext,
};
use rolldown_sourcemap::{MissingSourceMap, SourceMapOrMissing};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[derive(Debug)]
struct SourcemapBroken;

impl Plugin for SourcemapBroken {
  fn name(&self) -> Cow<'static, str> {
    "sourcemap-broken-transform".into()
  }

  async fn transform(
    &self,
    _ctx: &TransformPluginContext<'_>,
    _args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    Ok(Some(HookTransformOutput {
      code: None,
      map: Some(SourceMapOrMissing::Missing(MissingSourceMap {
        plugin_name: Some(self.name().into()),
      })),
      side_effects: None,
      module_type: None,
    }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_warn_if_hook_transform_map_is_undefined() {
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
