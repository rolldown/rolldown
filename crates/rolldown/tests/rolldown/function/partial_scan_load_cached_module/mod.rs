use std::{borrow::Cow, time::Duration};

use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem};
use rolldown_common::{ImportAttributes, ModuleDefFormat, ScanMode};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};

const ENTRY: &str = "virtual:entry";
const DEP: &str = "virtual:dep";

/// The entry calls `this.load` for its dependency. In a partial scan for the entry, the
/// dependency comes from the cache.
#[derive(Debug)]
struct LoadDependency {
  resolve_dependencies: bool,
}

impl Plugin for LoadDependency {
  fn name(&self) -> Cow<'static, str> {
    "load-dependency".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok([ENTRY, DEP].contains(&args.specifier).then(|| HookResolveIdOutput::from_id(args.specifier)))
  }

  async fn load(&self, ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let code = match args.id {
      ENTRY => {
        let attributes = ImportAttributes::default();
        let format = ModuleDefFormat::Unknown;
        ctx.load(DEP, attributes, None, format, self.resolve_dependencies, None).await?;
        "import { value } from 'virtual:dep'; console.log(value);"
      }
      DEP => "export const value = 42;",
      _ => return Ok(None),
    };
    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }
}

async fn assert_partial_scan_completes(resolve_dependencies: bool) {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: None, import: ENTRY.to_string() }]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(LoadDependency { resolve_dependencies })],
  )
  .unwrap();
  bundler.incremental_generate(ScanMode::Full).await.unwrap();

  tokio::time::timeout(
    Duration::from_secs(5),
    bundler.incremental_generate(ScanMode::Partial(vec![ENTRY.into()])),
  )
  .await
  .expect("`this.load` of a cached module hangs")
  .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_load_without_dependencies() {
  assert_partial_scan_completes(false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_load_with_dependencies() {
  assert_partial_scan_completes(true).await;
}
