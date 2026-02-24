use std::{borrow::Cow, sync::Arc};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

const VIRTUAL_HELPER: &str = "\0virtual:helper";

/// This plugin resolves `./helper` imports from data URL modules to a virtual module,
/// verifying that plugins are given a chance to resolve imports from data URL modules.
#[derive(Debug)]
struct DataUrlImportResolverPlugin;

impl Plugin for DataUrlImportResolverPlugin {
  fn name(&self) -> Cow<'static, str> {
    "data-url-import-resolver".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "./helper"
      && args.importer.is_some_and(|id| id.starts_with("\0rolldown/data-url:"))
    {
      return Ok(Some(HookResolveIdOutput { id: VIRTUAL_HELPER.into(), ..Default::default() }));
    }
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    if args.id == VIRTUAL_HELPER {
      return Ok(Some(HookLoadOutput {
        code: "export const value = 'from helper';".into(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }
}

/// Tests that plugins are given a chance to resolve imports from data URL modules.
/// If a plugin resolves the import, the build should succeed.
/// If no plugin resolves it, rolldown will throw an error (tested separately).
#[tokio::test(flavor = "multi_thread")]
async fn plugin_can_resolve_import_from_data_url_module() {
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./main.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Arc::new(DataUrlImportResolverPlugin)],
    )
    .await;
}
