use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use sugar_path::SugarPath;

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
    if args.id.ends_with("entry.js") {
      // Register both control files as watched transform dependencies, so changing them maps
      // to `entry.js` in the engine's default affected set.
      ctx.add_watch_file(ctx.cwd().join("./input.txt").normalize().to_str().unwrap());
      ctx.add_watch_file(ctx.cwd().join("./suppress.txt").normalize().to_str().unwrap());
    }
    Ok(None)
  }

  async fn hot_update(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookHotUpdateArgs,
  ) -> rolldown_plugin::HookHotUpdateReturn {
    if args.file.ends_with("input.txt") {
      // Replace the default affected set (`entry.js`, via the transform dependency) with
      // `dep.js` — the patch must ship dep's factory instead of entry's. Dep's own source is
      // untouched, so this also proves hook-selected modules bypass the unchanged-output
      // suppression (the changed information lives in `input.txt`, not in dep's code).
      assert_eq!(args.modules.len(), 1, "default set should be the transform-dep importer");
      assert!(args.modules[0].ends_with("entry.js"));
      let dep_id = ctx.cwd().join("./dep.js").normalize().to_str().unwrap().to_string();
      return Ok(Some(vec![dep_id.into()]));
    }
    if args.file.ends_with("suppress.txt") {
      // Suppress this file's update entirely — the step must produce a Noop update.
      return Ok(Some(vec![]));
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::HotUpdate
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn hot_update_hook() {
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
      vec![Arc::new(TestPlugin)],
    )
    .await;
}
