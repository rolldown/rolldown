use std::{borrow::Cow, sync::Arc};

use anyhow::Ok;
use rolldown::{BundlerOptions, InputItem};
use rolldown_common::SourceMapType;
use rolldown_plugin::{HookUsage, Plugin, PluginContext, SharedTransformPluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct TestLoadPlugin;

impl Plugin for TestLoadPlugin {
  fn name(&self) -> Cow<'static, str> {
    "test-load-plugin".into()
  }

  async fn load(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    Ok(Some(rolldown_plugin::HookLoadOutput {
      code: "".into(),
      side_effects: None,
      map: None,
      module_type: None,
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}

#[derive(Debug)]
struct TestTransformPlugin;

#[derive(Debug)]
struct TestRenderChunkPlugin;

impl Plugin for TestTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    "test-transform-plugin".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(args.code.clone()),
      map: None,
      side_effects: None,
      module_type: None,
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

impl Plugin for TestRenderChunkPlugin {
  fn name(&self) -> Cow<'static, str> {
    "test-render-chunk-plugin".into()
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    Ok(Some(rolldown_plugin::HookRenderChunkOutput { code: args.code.clone(), map: None }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn test() {
  manual_integration_test!()
    .build(TestMeta { ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("main".to_string()),
          import: "main.js".to_string(),
        }]),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      },
      vec![
        Arc::new(TestLoadPlugin),
        Arc::new(TestTransformPlugin),
        Arc::new(TestRenderChunkPlugin),
      ],
    )
    .await;
}
