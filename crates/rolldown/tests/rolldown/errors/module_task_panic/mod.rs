use std::{borrow::Cow, time::Duration};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookTransformArgs, HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct PanicInTransform;

impl Plugin for PanicInTransform {
  fn name(&self) -> Cow<'static, str> {
    "panic-in-transform".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    assert!(!args.id.ends_with("entry.js"), "panic inside a module task");
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_fail_the_build_instead_of_hanging() {
  let test =
    manual_integration_test!().build(TestMeta { expect_error: true, ..Default::default() });
  let build = test.run_with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      ..Default::default()
    },
    vec![Plugin::new_shared(PanicInTransform)],
  );
  tokio::time::timeout(Duration::from_secs(30), build)
    .await
    .expect("the build should not hang when a module task panics");
}
