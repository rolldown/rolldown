use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::RUNTIME_MODULE_KEY;
use rolldown_plugin::{HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// A plugin that removes `__exportAll` and `__toCommonJS` from the runtime module,
/// simulating a plugin that accidentally breaks runtime utilities.
#[derive(Debug)]
struct RuntimeBreakingPlugin;

impl Plugin for RuntimeBreakingPlugin {
  fn name(&self) -> Cow<'static, str> {
    "runtime-breaking-plugin".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.id == RUNTIME_MODULE_KEY {
      // Remove __exportAll and __toCommonJS from the runtime module
      let modified = args
        .code
        .replace("export var __exportAll", "var __removed_exportAll")
        .replace("export var __toCommonJS", "var __removed_toCommonJS");
      return Ok(Some(rolldown_plugin::HookTransformOutput {
        code: Some(modified),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

/// Test that removing a runtime utility function emits a proper error
/// instead of panicking.
#[tokio::test(flavor = "multi_thread")]
async fn runtime_module_symbol_removed_error() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(RuntimeBreakingPlugin)],
    )
    .await;
}
