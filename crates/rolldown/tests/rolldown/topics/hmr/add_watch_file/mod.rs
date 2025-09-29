use std::{borrow::Cow, fs, sync::Arc};

use rolldown::{BundlerOptions, ExperimentalOptions, HmrOptions, InputItem};
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
#[derive(Debug)]
struct TestPlugin;

impl Plugin for TestPlugin {
  fn name(&self) -> Cow<'static, str> {
    "TestPlugin".into()
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let input_text_path = ctx.cwd().join("./input.txt");
    let content = fs::read_to_string(&input_text_path).unwrap();
    ctx.add_watch_file(input_text_path.to_str().unwrap());
    let new_code = args.code.replace(
      "import.meta.getContent('./input.txt')",
      &format!("\"{}\"", content.replace('\n', "\\n")),
    );
    Ok(Some(rolldown_plugin::HookTransformOutput { code: Some(new_code), ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn add_watch_file() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        experimental: Some(ExperimentalOptions {
          hmr: Some(HmrOptions::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Arc::new(TestPlugin)],
    )
    .await;
}
