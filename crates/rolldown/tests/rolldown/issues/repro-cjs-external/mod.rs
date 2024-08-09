use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::ImportKind;
use rolldown_plugin::{
  HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn,
  Plugin, SharedPluginContext,
};
use rolldown_testing::{abs_file_dir, integration_test::IntegrationTest, test_config::TestMeta};

// cargo test -p rolldown --test integration_rolldown repro

#[derive(Debug)]
struct CjsExternalPlugin {
  externals: Vec<String>,
}

const CJS_EXTERNAL_FACADE_NAMESPACE: &str = "vite:cjs-external-facade";

// https://github.com/rolldown/vite/pull/24
// https://github.com/evanw/esbuild/issues/566#issuecomment-735551834

impl Plugin for CjsExternalPlugin {
  fn name(&self) -> Cow<'static, str> {
    "cjs-external".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if self.externals.iter().any(|e| args.specifier == e) {
      if matches!(args.kind, ImportKind::Require) {
        return Ok(Some(HookResolveIdOutput {
          id: CJS_EXTERNAL_FACADE_NAMESPACE.to_string() + args.specifier,
          ..Default::default()
        }));
      }

      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.to_string(),
        external: Some(true),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    if args.id.starts_with(CJS_EXTERNAL_FACADE_NAMESPACE) {
      let id = args.id.strip_prefix(CJS_EXTERNAL_FACADE_NAMESPACE).unwrap();
      let code = format!(r#"import * as m from "{id}"; module.exports = m;"#);
      return Ok(Some(HookLoadOutput { code, ..Default::default() }));
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
      vec![Arc::new(CjsExternalPlugin { externals: vec!["react".to_string()] })],
    )
    .await;
}
