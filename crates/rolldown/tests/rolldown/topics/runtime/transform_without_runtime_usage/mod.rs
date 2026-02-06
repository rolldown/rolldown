use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::RUNTIME_MODULE_KEY;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Plugin that injects a side-effectful `console.log` into the runtime module,
/// verifying that transformed runtime content is preserved even when the entry
/// code uses no runtime features (pure ESM).
#[derive(Debug)]
struct TransformWithoutRuntimeUsagePlugin;

impl Plugin for TransformWithoutRuntimeUsagePlugin {
  fn name(&self) -> Cow<'static, str> {
    "transform-without-runtime-usage-plugin".into()
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id == RUNTIME_MODULE_KEY {
      let mut code = args.code.clone();
      code.push_str("\nconsole.log(\"transform-without-usage\");\n");
      return Ok(Some(rolldown_plugin::HookTransformOutput {
        code: Some(code),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn transform_runtime_module_without_usage() {
  let plugin = Arc::new(TransformWithoutRuntimeUsagePlugin);

  manual_integration_test!()
    .build(TestMeta { hidden_runtime_module: false, ..Default::default() })
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
