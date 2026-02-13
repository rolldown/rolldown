use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, Plugin, PluginContext, RegisterHook,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use sugar_path::SugarPath;

#[derive(Debug)]
struct ExternalCss;

#[RegisterHook]
impl Plugin for ExternalCss {
  fn name(&self) -> Cow<'static, str> {
    "external-css".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier.as_path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("css")) {
      let path = format!("rewritten-{}", args.specifier);
      return Ok(Some(HookResolveIdOutput {
        id: path.into(),
        external: Some(true.into()),
        ..Default::default()
      }));
    }
    Ok(None)
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_rewrite_dynamic_imports_that_import_external_modules() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(ExternalCss)],
    )
    .await;
}
