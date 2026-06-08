use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::Arc,
};

use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct SlashNormalizedResolvePlugin;

impl SlashNormalizedResolvePlugin {
  fn resolve_file(cwd: &Path, args: &HookResolveIdArgs<'_>) -> Option<PathBuf> {
    let specifier = Path::new(args.specifier);
    let unresolved = if specifier.is_absolute() {
      specifier.to_path_buf()
    } else {
      let base = args.importer.and_then(|importer| Path::new(importer).parent()).unwrap_or(cwd);
      base.join(specifier)
    };

    if unresolved.extension().is_some() && unresolved.is_file() {
      return dunce::canonicalize(unresolved).ok();
    }

    ["ts", "js", "json"].into_iter().find_map(|extension| {
      let candidate = unresolved.with_extension(extension);
      candidate.is_file().then(|| dunce::canonicalize(candidate).ok()).flatten()
    })
  }
}

impl Plugin for SlashNormalizedResolvePlugin {
  fn name(&self) -> Cow<'static, str> {
    "slash-normalized-resolve".into()
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    let Some(path) = Self::resolve_file(ctx.cwd(), args) else {
      return Ok(None);
    };
    let id = path.to_string_lossy().replace('\\', "/");
    Ok(Some(HookResolveIdOutput::from_id(id)))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn preserve_modules_root_with_slash_normalized_ids() {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: None, import: "./src/bin/index.ts".into() }]),
        format: Some(OutputFormat::Esm),
        preserve_modules: Some(true),
        preserve_modules_root: Some("src".into()),
        ..Default::default()
      },
      vec![Arc::new(SlashNormalizedResolvePlugin)],
    )
    .await;
}
