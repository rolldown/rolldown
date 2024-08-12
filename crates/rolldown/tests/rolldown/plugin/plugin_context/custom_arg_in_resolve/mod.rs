use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  typedmap::{TypedDashMap, TypedMapKey},
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, Plugin, PluginContext,
  PluginContextResolveOptions,
};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};
#[derive(Debug)]
struct TestPluginCaller;

#[derive(Hash, PartialEq, Eq)]
struct MyArg {
  id: usize,
}

impl TypedMapKey for MyArg {
  type Value = String;
}

impl Plugin for TestPluginCaller {
  fn name(&self) -> Cow<'static, str> {
    "TestPluginCaller".into()
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "foo" {
      let custom = TypedDashMap::default();
      custom.insert(MyArg { id: 0 }, "hello, world".to_string());
      let custom_resolve_ret = ctx
        .resolve(
          "test",
          None,
          Some(PluginContextResolveOptions { custom: Arc::new(custom), ..Default::default() }),
        )
        .await??;

      if custom_resolve_ret.id == "hello, world" {
        Ok(Some(HookResolveIdOutput {
          id: "hello, world".to_string(),
          external: Some(true),
          ..Default::default()
        }))
      } else {
        panic!("test")
      }
    } else {
      Ok(None)
    }
  }
}

#[derive(Debug)]
struct TestPluginReceiver;

impl Plugin for TestPluginReceiver {
  fn name(&self) -> Cow<'static, str> {
    "TestPluginReceiver".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if let Some(value) = args.custom.get::<MyArg>(&MyArg { id: 0 }) {
      assert_eq!(value.as_str(), "hello, world");
      return Ok(Some(HookResolveIdOutput { id: value.to_string(), ..Default::default() }));
    }
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_pass_custom_arg() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(TestPluginCaller), Arc::new(TestPluginReceiver)],
    )
    .await;
}
