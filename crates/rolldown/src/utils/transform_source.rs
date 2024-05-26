use anyhow::Result;
use rolldown_common::{side_effects::HookSideEffects, ResolvedPath};
use rolldown_plugin::{HookTransformArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;

pub async fn transform_source(
  plugin_driver: &PluginDriver,
  resolved_path: &ResolvedPath,
  source: String,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
) -> Result<String> {
  plugin_driver
    .transform(
      &HookTransformArgs { id: &resolved_path.path, code: &source },
      sourcemap_chain,
      side_effects,
      &source,
    )
    .await
}
