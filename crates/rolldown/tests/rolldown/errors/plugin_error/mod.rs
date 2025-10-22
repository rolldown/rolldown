use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
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
  ) -> rolldown_plugin::HookNoopReturn {
    Err::<_, BuildDiagnostic>(anyhow::anyhow!("A").into())
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
