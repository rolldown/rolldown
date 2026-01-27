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

/// Test that plugins can transform the runtime module via the transform hook.
#[derive(Debug)]
struct RuntimeTransformPlugin {
  transform_called: Arc<AtomicBool>,
}

impl Plugin for RuntimeTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    "runtime-transform-plugin".into()
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    // Reset at the start of each build
    self.transform_called.store(false, Ordering::SeqCst);
    Ok(())
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id == RUNTIME_MODULE_KEY {
      self.transform_called.store(true, Ordering::SeqCst);
    }
    Ok(None)
  }

  async fn build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> HookNoopReturn {
    // Return error if transform was not called for the runtime module
    if !self.transform_called.load(Ordering::SeqCst) {
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
  let transform_called = Arc::new(AtomicBool::new(false));
  let plugin = Arc::new(RuntimeTransformPlugin { transform_called: Arc::clone(&transform_called) });

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
