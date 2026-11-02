use std::{borrow::Cow, path::Path, time::Duration};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{IsExternal, ModuleDefFormat};
use rolldown_plugin::{
  HookTransformArgs, HookTransformReturn, HookUsage, Plugin, SharedTransformPluginContext,
};
use rolldown_std_utils::PathExt as _;
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// This plugin calls `this.load` for lib.js. The `./lib.js` import in main.js already made
/// that id external.
#[derive(Debug)]
struct LoadExternalId;

impl Plugin for LoadExternalId {
  fn name(&self) -> Cow<'static, str> {
    "load-external-id".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    let id = Path::new(args.id);
    if id.ends_with("trigger.js") {
      ctx.load(id.with_file_name("lib.js").expect_to_str(), None, ModuleDefFormat::Unknown).await?;
    }
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_load_of_external_id_is_an_error() {
  // An external module never sends the load signal that `this.load` waits for. Before the fix,
  // this build did not end.
  tokio::time::timeout(
    Duration::from_secs(60),
    manual_integration_test!()
      .build(TestMeta { expect_error: true, ..Default::default() })
      .run_with_plugins(
        BundlerOptions {
          input: Some(vec![InputItem {
            name: Some("main".to_string()),
            import: "./main.js".to_string(),
          }]),
          external: Some(IsExternal::from(vec!["./lib.js".to_string()])),
          ..Default::default()
        },
        vec![Plugin::new_shared(LoadExternalId)],
      ),
  )
  .await
  .expect("`this.load` of an external id must not hang the build");
}
