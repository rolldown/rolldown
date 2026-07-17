use oxc::semantic::NodeId;
use rolldown_common::{ModuleIdx, RolldownFileUrlReference};
use rolldown_plugin::{HookResolveFileUrlArgs, HookResolveFileUrlOutput};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

/// Plugin-supplied replacements for `import.meta.ROLLDOWN_FILE_URL_<referenceId>`,
/// keyed by the module and the `NodeId` of the member expression being replaced.
///
/// The code is unparsed: the module finalizer parses it once, into that module's arena.
/// Each entry carries the plugin that produced it so a parse failure there can be
/// attributed without the driver having to parse the code itself.
pub type ResolvedFileUrls = FxHashMap<(ModuleIdx, NodeId), HookResolveFileUrlOutput>;

impl GenerateStage<'_> {
  /// Calls the `resolveFileUrl` hook for every recorded occurrence.
  ///
  /// Runs after preliminary chunk filenames are assigned and before
  /// `finalize_modules`, which is sync and rayon-parallel and therefore cannot call
  /// plugin hooks itself.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) async fn resolve_file_urls(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> anyhow::Result<ResolvedFileUrls> {
    let mut resolved = FxHashMap::default();

    let has_hook = !self.plugin_driver.order_by_resolve_file_url_meta.is_empty();
    if !has_hook {
      return Ok(resolved);
    }

    let out_dir = self.options.cwd.as_path().join(&self.options.out_dir);

    // Traverse the pre-sorted chunks and their modules so `resolveFileUrl` calls
    // do not depend on module discovery order.
    for chunk_idx in &chunk_graph.sorted_chunk_idx_vec {
      let chunk = &chunk_graph.chunk_table[*chunk_idx];
      if chunk.modules.is_empty() {
        continue;
      }
      let chunk_id = chunk
        .preliminary_filename
        .as_ref()
        .expect("chunk should have a preliminary filename by now")
        .as_str();

      for module_idx in &chunk.modules {
        let Some(module) = self.link_output.module_table[*module_idx].as_normal() else {
          continue;
        };
        if !self.link_output.metas[module.idx].is_included
          || module.ecma_view.rolldown_file_url_references.is_empty()
        {
          continue;
        }

        for RolldownFileUrlReference { node_id, stmt_info_idx, reference_id } in
          &module.ecma_view.rolldown_file_url_references
        {
          if !self.link_output.metas[module.idx].stmt_info_included.has_bit(*stmt_info_idx) {
            continue;
          }
          // Unknown reference ids are handled in the rewrite
          let Ok(file_name) = self.plugin_driver.file_emitter.get_file_name(reference_id) else {
            continue;
          };

          let output = if has_hook {
            let absolute = file_name.as_path().absolutize_with(&out_dir);
            let relative_path = chunk.relative_path_for(&absolute);

            let args = HookResolveFileUrlArgs {
              chunk_id,
              file_name: &file_name,
              format: self.options.format,
              module_id: module.id.as_str(),
              reference_id,
              relative_path: &relative_path,
            };
            self.plugin_driver.resolve_file_url(&args).await?
          } else {
            None
          };

          if let Some(output) = output {
            resolved.insert((module.idx, *node_id), output);
          }
        }
      }
    }

    Ok(resolved)
  }
}
