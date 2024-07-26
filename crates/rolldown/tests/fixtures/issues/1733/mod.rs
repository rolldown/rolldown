use std::{borrow::Cow, sync::Arc};

use rolldown::{Bundler, BundlerOptions};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, Plugin, SharedPluginContext,
};
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
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec!["./entry.js".to_string().into()]),
      cwd: Some(cwd.clone()),
      ..Default::default()
    },
    vec![Arc::new(ExternalCss)],
  );

  let output = bundler.write().await.unwrap();

  assert!(output.errors.is_empty(), "{:?}", output.errors);

  insta::assert_snapshot!(rolldown_testing::utils::stringify_bundle_output(output, &cwd));
}
