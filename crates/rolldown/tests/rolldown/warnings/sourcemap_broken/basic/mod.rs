use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, SharedPluginContext,
};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[derive(Debug)]
struct SourcemapBroken;

impl Plugin for SourcemapBroken {
  fn name(&self) -> Cow<'static, str> {
    "sourcemap-broken-basic".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "test.js" {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        external: Some(false),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn load(&self, _ctx: &SharedPluginContext, _args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok(Some(HookLoadOutput { code: format!("export default {{}}"), ..Default::default() }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_failed_to_resolve_the_module_with_diagnostic() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(SourcemapBroken)],
    )
    .await;
}
