use rolldown_common::{Chunk, ChunkKind, PreRenderedChunk, RenderedChunk, RenderedModule};
use rustc_hash::FxHashMap;

use crate::{stages::link_stage::LinkStageOutput, SharedOptions};

use self::render_chunk_exports::get_chunk_export_names;

pub mod deconflict_chunk_symbols;
pub mod finalize_chunks;
pub mod render_chunk;
pub mod render_chunk_exports;
pub mod render_chunk_imports;

pub fn generate_pre_rendered_chunk(
  chunk: &Chunk,
  graph: &LinkStageOutput,
  output_options: &SharedOptions,
) -> PreRenderedChunk {
  PreRenderedChunk {
    is_entry: matches!(&chunk.kind, ChunkKind::EntryPoint { is_user_defined, .. } if *is_user_defined),
    is_dynamic_entry: matches!(&chunk.kind, ChunkKind::EntryPoint { is_user_defined, .. } if !*is_user_defined),
    facade_module_id: match &chunk.kind {
      ChunkKind::EntryPoint { module, .. } => {
        Some(graph.module_table.normal_modules[*module].resource_id.expect_file().to_string())
      }
      ChunkKind::Common => None,
    },
    module_ids: chunk
      .modules
      .iter()
      .map(|id| graph.module_table.normal_modules[*id].resource_id.expect_file().to_string())
      .collect(),
    exports: get_chunk_export_names(chunk, graph, output_options),
  }
}

pub fn generate_rendered_chunk(
  chunk: &Chunk,
  graph: &LinkStageOutput,
  output_options: &SharedOptions,
  render_modules: FxHashMap<String, RenderedModule>,
) -> RenderedChunk {
  let pre_rendered_chunk = generate_pre_rendered_chunk(chunk, graph, output_options);
  RenderedChunk {
    is_entry: pre_rendered_chunk.is_entry,
    is_dynamic_entry: pre_rendered_chunk.is_dynamic_entry,
    facade_module_id: pre_rendered_chunk.facade_module_id,
    module_ids: pre_rendered_chunk.module_ids,
    exports: pre_rendered_chunk.exports,
    file_name: chunk
      .preliminary_filename
      .as_ref()
      .expect("should have preliminary_filename")
      .to_string(),
    modules: render_modules,
  }
}
