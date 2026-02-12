use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::RUNTIME_MODULE_KEY;
use rolldown_plugin::{HookBuildStartArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Per-build state stored in PluginContextMeta to avoid race conditions
/// when plugins are reused across multiple concurrent builds.
#[derive(Debug, Default)]
struct RuntimeTransformState {
  transform_called: AtomicBool,
}

/// Test that plugins can transform the runtime module via the transform hook.
#[derive(Debug)]
struct RuntimeTransformPlugin;

impl Plugin for RuntimeTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    "runtime-transform-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    // Initialize per-build state in context meta
    ctx.meta().insert(Arc::new(RuntimeTransformState::default()));
    Ok(())
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id == RUNTIME_MODULE_KEY {
      let state = ctx.meta().get::<RuntimeTransformState>().ok_or_else(|| {
        anyhow::anyhow!("RuntimeTransformState not found - build_start may not have been called")
      })?;
      state.transform_called.store(true, Ordering::SeqCst);
    }
    Ok(None)
  }

  async fn build_end(
    &self,
    ctx: &PluginContext,
    _args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> HookNoopReturn {
    // Get per-build state from context meta
    let state = ctx.meta().get::<RuntimeTransformState>().ok_or_else(|| {
      anyhow::anyhow!("RuntimeTransformState not found - build_start may not have been called")
    })?;
    // Return error if transform was not called for the runtime module
    if !state.transform_called.load(Ordering::SeqCst) {
      return Err(anyhow::anyhow!("Transform hook was not called for the runtime module"));
    }
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::Transform | HookUsage::BuildEnd
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn transform_runtime_module() {
  let plugin = Arc::new(RuntimeTransformPlugin);

  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![plugin],
    )
    .await;
}
