use rolldown_common::{ChunkKind, RenderedChunk, RenderedModule};
use rustc_hash::FxHashMap;

use crate::{stages::link_stage::LinkStageOutput, OutputOptions};

use super::Chunk;

#[derive(Debug, Clone)]
pub struct PreRenderedChunk {
  // pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
}

impl Chunk {
  pub fn get_pre_rendered_chunk_info(
    &self,
    graph: &LinkStageOutput,
    output_options: &OutputOptions,
  ) -> PreRenderedChunk {
    PreRenderedChunk {
      is_entry: matches!(&self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if *is_user_defined),
      is_dynamic_entry: matches!(&self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if !*is_user_defined),
      facade_module_id: match &self.kind {
        ChunkKind::EntryPoint { module, .. } => {
          Some(graph.module_table.normal_modules[*module].resource_id.expect_file().to_string())
        }
        ChunkKind::Common => None,
      },
      module_ids: self
        .modules
        .iter()
        .map(|id| graph.module_table.normal_modules[*id].resource_id.expect_file().to_string())
        .collect(),
      exports: self.get_export_names(graph, output_options),
    }
  }

  pub fn get_rendered_chunk_info(
    &self,
    graph: &LinkStageOutput,
    output_options: &OutputOptions,
    render_modules: FxHashMap<String, RenderedModule>,
  ) -> RenderedChunk {
    let pre_rendered_chunk = self.get_pre_rendered_chunk_info(graph, output_options);
    RenderedChunk {
      is_entry: pre_rendered_chunk.is_entry,
      is_dynamic_entry: pre_rendered_chunk.is_dynamic_entry,
      facade_module_id: pre_rendered_chunk.facade_module_id,
      module_ids: pre_rendered_chunk.module_ids,
      exports: pre_rendered_chunk.exports,
      file_name: self.file_name.clone().expect("should have file name"),
      modules: render_modules,
    }
  }
}
