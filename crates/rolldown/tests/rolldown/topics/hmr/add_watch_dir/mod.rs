use std::{borrow::Cow, fs};

use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
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
    let content_dir = ctx.cwd().join("content");
    let content = fs::read_to_string(content_dir.join("input.txt")).unwrap();
    ctx.add_watch_file(content_dir.to_str().unwrap());
    let new_code = args.code.replace(
      "import.meta.getContent('./content/input.txt')",
      &format!("\"{}\"", content.replace('\n', "\\n")),
    );
    Ok(Some(rolldown_plugin::HookTransformOutput { code: Some(new_code), ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn add_watch_dir() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        experimental: Some(ExperimentalOptions {
          dev_mode: Some(DevModeOptions::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Plugin::new_shared(TestPlugin)],
    )
    .await;
}
