use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_error::BatchedBuildDiagnostic;
use rolldown_plugin::{
  HookLoadReturn, HookNoopReturn, HookResolveIdReturn, HookTransformReturn, HookUsage, Plugin,
  PluginContext, SharedLoadPluginContext, SharedTransformPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct PluginErrorTest;

impl Plugin for PluginErrorTest {
  fn name(&self) -> Cow<'static, str> {
    "plugin-error-test".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    Err(BatchedBuildDiagnostic::new(vec![
      anyhow::anyhow!("A").into(),
      anyhow::anyhow!("B").into(),
    ]))?
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_emit_multi_plugin_diagnostics() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(PluginErrorTest)],
    )
    .await;
}

// Tests that errors thrown from the `load` hook are properly caught and reported
// as build errors (not as a panic or unhandled error).
// Regression test for https://github.com/rolldown/rolldown/issues/4387
#[derive(Debug)]
struct LoadHookErrorPlugin;

impl Plugin for LoadHookErrorPlugin {
  fn name(&self) -> Cow<'static, str> {
    "load-hook-error".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }

  async fn load(
    &self,
    _ctx: SharedLoadPluginContext,
    _args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    Err(anyhow::anyhow!("load hook error from plugin"))?
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_catch_error_thrown_in_load_hook() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(LoadHookErrorPlugin)],
    )
    .await;
}

// Tests that errors thrown from the `resolve_id` hook are properly caught.
#[derive(Debug)]
struct ResolveIdHookErrorPlugin;

impl Plugin for ResolveIdHookErrorPlugin {
  fn name(&self) -> Cow<'static, str> {
    "resolve-id-hook-error".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Err(anyhow::anyhow!("resolve_id hook error from plugin"))?
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_catch_error_thrown_in_resolve_id_hook() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(ResolveIdHookErrorPlugin)],
    )
    .await;
}

// Tests that errors thrown from the `transform` hook are properly caught.
#[derive(Debug)]
struct TransformHookErrorPlugin;

impl Plugin for TransformHookErrorPlugin {
  fn name(&self) -> Cow<'static, str> {
    "transform-hook-error".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    _args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    Err(anyhow::anyhow!("transform hook error from plugin"))?
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_catch_error_thrown_in_transform_hook() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(TransformHookErrorPlugin)],
    )
    .await;
}
