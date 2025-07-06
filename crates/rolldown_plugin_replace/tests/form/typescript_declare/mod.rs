use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin::{
  HookTransformArgs, HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext,
};
use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use std::sync::Mutex;

#[derive(Debug)]
struct TestPlugin(Arc<Mutex<Option<String>>>);

impl Plugin for TestPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "test-plugin".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    let mut code = self.0.lock().unwrap();
    *code = Some(args.code.clone());
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

// doesn't replace lvalue in typescript declare
#[tokio::test(flavor = "multi_thread")]
async fn typescript_declare() {
  let code: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

  manual_integration_test!()
    .build(TestMeta { expect_executed: false, visualize_sourcemap: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions { input: Some(vec!["./input.ts".to_string().into()]), ..Default::default() },
      vec![
        Arc::new(ReplacePlugin::with_options(ReplaceOptions {
          values: std::iter::once(("NAME".to_string(), "replaced".to_string())).collect(),
          prevent_assignment: true,
          sourcemap: true,
          ..Default::default()
        })),
        Arc::new(TestPlugin(Arc::clone(&code))),
      ],
    )
    .await;

  let replaced = "declare const NAME: string;\nconsole.log(replaced);\n";
  assert_eq!(*code.lock().unwrap().as_ref().unwrap(), replaced);
}
