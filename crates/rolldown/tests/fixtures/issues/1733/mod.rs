use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, Plugin, SharedPluginContext,
};
use rolldown_testing::{integration_test::IntegrationTest, test_config::TestMeta};
use sugar_path::SugarPath;

#[derive(Debug)]
struct ExternalCss;

#[async_trait::async_trait]
impl Plugin for ExternalCss {
  fn name(&self) -> Cow<'static, str> {
    "external-css".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    if args.source.as_path().extension().map_or(false, |ext| ext.eq_ignore_ascii_case("css")) {
      let path = format!("rewritten-{}", args.source);
      return Ok(Some(HookResolveIdOutput {
        id: path,
        external: Some(true),
        ..Default::default()
      }));
    }
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_rewrite_dynamic_imports_that_import_external_modules() {
  let cwd = file!()
    .as_path()
    .parent()
    .unwrap()
    .to_path_buf()
    .absolutize_with(env!("WORKSPACE_DIR").as_path());

  IntegrationTest::new(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        cwd: Some(cwd.clone()),
        ..Default::default()
      },
      vec![Arc::new(ExternalCss)],
    )
    .await;
}
