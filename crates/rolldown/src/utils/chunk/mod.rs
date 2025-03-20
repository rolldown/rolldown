use rolldown_common::{
  Chunk, ChunkKind, ModuleId, RenderedModule, RollupPreRenderedChunk, RollupRenderedChunk,
};
use rustc_hash::FxHashMap;

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

use self::render_chunk_exports::get_chunk_export_names;

pub mod deconflict_chunk_symbols;
pub mod determine_export_mode;
pub mod determine_use_strict;
pub mod finalize_chunks;
pub mod namespace_marker;
pub mod render_chunk_exports;
pub mod validate_options_for_multi_chunk_output;

pub fn generate_pre_rendered_chunk(
  chunk: &Chunk,
  graph: &LinkStageOutput,
) -> RollupPreRenderedChunk {
  RollupPreRenderedChunk {
    name: chunk.name.clone().expect("should have name"),
    is_entry: matches!(&chunk.kind, ChunkKind::EntryPoint { is_user_defined, .. } if *is_user_defined),
    is_dynamic_entry: matches!(&chunk.kind, ChunkKind::EntryPoint { is_user_defined, .. } if !*is_user_defined),
    facade_module_id: match &chunk.kind {
      ChunkKind::EntryPoint { module, .. } => Some(graph.module_table.modules[*module].id().into()),
      ChunkKind::Common => None,
    },
    module_ids: chunk
      .modules
      .iter()
      .map(|id| graph.module_table.modules[*id].id().into())
      .collect(),
    exports: get_chunk_export_names(chunk, graph),
  }
}

pub fn generate_rendered_chunk(
  chunk: &Chunk,
  render_modules: FxHashMap<ModuleId, RenderedModule>,
  pre_rendered_chunk: &RollupPreRenderedChunk,
  chunk_graph: &ChunkGraph,
) -> RollupRenderedChunk {
  RollupRenderedChunk {
    name: pre_rendered_chunk.name.clone(),
    is_entry: pre_rendered_chunk.is_entry,
    is_dynamic_entry: pre_rendered_chunk.is_dynamic_entry,
    facade_module_id: pre_rendered_chunk.facade_module_id.clone(),
    module_ids: pre_rendered_chunk.module_ids.clone(),
    exports: pre_rendered_chunk.exports.clone(),
    filename: chunk
      .preliminary_filename
      .as_deref()
      .expect("should have preliminary_filename")
      .clone(),
    modules: render_modules.into(),
    imports: chunk
      .cross_chunk_imports
      .iter()
      .map(|id| {
        chunk_graph.chunk_table[*id]
          .preliminary_filename
          .as_deref()
          .expect("should have preliminary_filename")
          .clone()
      })
      .collect(),
    dynamic_imports: chunk
      .cross_chunk_dynamic_imports
      .iter()
      .map(|id| {
        chunk_graph.chunk_table[*id]
          .preliminary_filename
          .as_deref()
          .expect("should have preliminary_filename")
          .clone()
      })
      .collect(),
  }
}
