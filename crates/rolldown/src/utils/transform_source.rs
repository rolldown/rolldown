use std::sync::Arc;
use std::sync::mpsc::Sender;

use anyhow::Result;
use rolldown_common::ModuleType;
use rolldown_common::NormalizedBundlerOptions;
use rolldown_common::SourceMapGenMsg;
use rolldown_common::{
  ModuleIdx, ResolvedId, SourcemapChainElement, side_effects::HookSideEffects,
};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::PluginDriver;

#[inline]
#[tracing::instrument(level = "debug", skip_all)]
#[expect(clippy::too_many_arguments)]
pub async fn transform_source(
  plugin_driver: &PluginDriver,
  options: &NormalizedBundlerOptions,
  resolved_id: &ResolvedId,
  module_idx: ModuleIdx,
  source: String,
  sourcemap_chain: &mut Vec<SourcemapChainElement>,
  side_effects: &mut Option<HookSideEffects>,
  module_type: &mut ModuleType,
  magic_string_tx: Option<Arc<Sender<SourceMapGenMsg>>>,
  warnings: &mut Vec<BuildDiagnostic>,
) -> Result<String> {
  let mut plugin_names: Vec<String> = vec![];
  let result = plugin_driver
    .transform(
      &resolved_id.id,
      module_idx,
      source,
      sourcemap_chain,
      side_effects,
      module_type,
      magic_string_tx,
      &mut plugin_names,
    )
    .await;
  if options.sourcemap.is_some() {
    let sourcemap_type =
      options.sourcemap.as_ref().map(rolldown_common::SourceMapType::as_str).unwrap();
    if sourcemap_type == "file" || sourcemap_type == "inline" {
      if !plugin_names.is_empty() {
        plugin_names.iter().for_each(|plugin_name| {
          warnings.push(
            BuildDiagnostic::sourcemap_broken(plugin_name.as_str(), "transform", sourcemap_type)
              .with_severity_warning(),
          );
        });
      }
    }
  }
  result
}
