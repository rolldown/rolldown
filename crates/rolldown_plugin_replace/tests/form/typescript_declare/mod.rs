use std::sync::Arc;

use rolldown::BundlerOptions;

use rolldown_plugin::{
  HookTransformArgs, HookTransformReturn, Plugin, SharedTransformPluginContext,
};
use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};
use std::sync::Mutex;

#[derive(Debug)]
struct TestPlugin(Arc<Mutex<Option<String>>>);

impl Plugin for TestPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "test-plugin".into()
  }

  fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    let mut code = self.0.lock().unwrap();
    *code = Some(args.code.clone());
    async { Ok(None) }
  }
}

// doesn't replace lvalue in typescript declare
#[tokio::test(flavor = "multi_thread")]
async fn typescript_declare() {
  let cwd = abs_file_dir!();
  let code = Arc::new(Mutex::new(None));

  IntegrationTest::new(TestMeta {
    expect_executed: false,
    visualize_sourcemap: true,
    ..Default::default()
  })
  .run_with_plugins(
    BundlerOptions {
      input: Some(vec!["./input.ts".to_string().into()]),
      cwd: Some(cwd),
      ..Default::default()
    },
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
