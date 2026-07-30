use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem, RepresentType};
use rolldown_common::{ModuleInfo, NormalModule};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookNoopReturn, HookTransformArgs,
  HookTransformOutput, HookTransformReturn, HookUsage, Plugin, PluginContext,
  SharedLoadPluginContext, SharedTransformPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct TestPlugin;

impl Plugin for TestPlugin {
  fn name(&self) -> Cow<'static, str> {
    "represent-type-test".into()
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, _args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok(Some(HookLoadOutput {
      code: "export default 1".into(),
      represent_type: Some(RepresentType::Text),
      ..Default::default()
    }))
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    _args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    Ok(Some(HookTransformOutput {
      represent_type: Some(RepresentType::Base64),
      ..Default::default()
    }))
  }

  async fn module_parsed(
    &self,
    _ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> HookNoopReturn {
    assert_eq!(module_info.represent_type, Some(RepresentType::Base64));
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load | HookUsage::Transform | HookUsage::ModuleParsed
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn transform_represent_type_overrides_load_metadata() {
  manual_integration_test!()
    .build(TestMeta {
      snapshot: false,
      write_to_disk: false,
      expect_executed: false,
      ..Default::default()
    })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: None, import: "./entry.js".to_string() }]),
        ..Default::default()
      },
      vec![Arc::new(TestPlugin)],
    )
    .await;
}
