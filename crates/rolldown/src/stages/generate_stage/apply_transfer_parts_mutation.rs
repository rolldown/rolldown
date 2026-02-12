use std::sync::Arc;

use rolldown_common::{
  ImportRecordIdx, ModuleIdx, PrependRenderedImport, RenderedConcatenatedModuleParts,
};
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn apply_transfer_parts_mutation(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    transfer_parts_rendered_maps: Vec<(
      ModuleIdx,
      FxIndexMap<ImportRecordIdx, String>,
      RenderedConcatenatedModuleParts,
    )>,
  ) {
    let mut normalized_transfer_parts_rendered_maps = FxHashMap::default();
    for (idx, transferred_import_record, rendered_concatenated_module_parts) in
      transfer_parts_rendered_maps
    {
      for (rec_idx, rendered_string) in transferred_import_record {
        normalized_transfer_parts_rendered_maps.insert((idx, rec_idx), rendered_string);
      }
      let chunk_idx = chunk_graph.module_to_chunk[idx].expect("should have chunk idx");
      let chunk = &mut chunk_graph.chunk_table[chunk_idx];
      chunk
        .module_idx_to_render_concatenated_module
        .insert(idx, rendered_concatenated_module_parts);
    }

    if normalized_transfer_parts_rendered_maps.is_empty() {
      return;
    }
    for chunk in chunk_graph.chunk_table.iter_mut() {
      for (module_idx, recs) in &chunk.insert_map {
        let Some(module) = self.link_output.module_table[*module_idx].as_normal_mut() else {
          continue;
        };
        for (importer_idx, rec_idx) in recs {
          if let Some(rendered_string) =
            normalized_transfer_parts_rendered_maps.get(&(*importer_idx, *rec_idx))
          {
            module
              .ecma_view
              .mutations
              .push(Arc::new(PrependRenderedImport { intro: rendered_string.clone() }));
          }
        }
      }
    }
  }
}
