use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, Plugin, SharedPluginContext,
};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};
use sugar_path::SugarPath;

#[derive(Debug)]
struct Basic;

impl Plugin for Basic {
  fn name(&self) -> Cow<'static, str> {
    "basic".into()
  }

  // rewrite `lib` -> './lib.js'
  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    let cwd = abs_file_dir!();
    if args.specifier == "lib" {
      return Ok(Some(HookResolveIdOutput {
        id: cwd.join("lib.js").to_slash_lossy().to_string(),
        external: Some(false),
        ..Default::default()
      }));
    }
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_rewrite_dynamic_imports_that_import_external_modules() {
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
      vec![Arc::new(Basic)],
    )
    .await;
}
