use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

#[derive(Debug)]
struct UnresolvedImport;

impl Plugin for UnresolvedImport {
  fn name(&self) -> Cow<'static, str> {
    "unresolved-import-basic".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "test.js" {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: Some(false.into()),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}

#[ignore = "https://github.com/rolldown/rolldown/pull/2006#issuecomment-2294898310"]
#[tokio::test(flavor = "multi_thread")]
async fn should_failed_to_resolve_the_module_with_diagnostic() {
  let cwd = abs_file_dir!();

  IntegrationTest::new(TestMeta { expect_error: true, ..Default::default() }, abs_file_dir!())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        cwd: Some(cwd),
        ..Default::default()
      },
      vec![Arc::new(UnresolvedImport)],
    )
    .await;
}
