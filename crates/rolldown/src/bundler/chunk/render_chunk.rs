use rolldown_common::EntryPointKind;
use rustc_hash::FxHashMap;

use crate::{bundler::stages::link_stage::LinkStageOutput, OutputOptions, RenderedModule};

use super::chunk::Chunk;

#[derive(Debug, Clone)]
pub struct PreRenderedChunk {
  // pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RenderedChunk {
  // PreRenderedChunk
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub file_name: String,
  pub modules: FxHashMap<String, RenderedModule>,
}

impl Chunk {
  pub fn get_pre_rendered_chunk_info(
    &self,
    graph: &LinkStageOutput,
    output_options: &OutputOptions,
  ) -> PreRenderedChunk {
    PreRenderedChunk {
      is_entry: matches!(&self.entry_point, Some(e) if e.kind == EntryPointKind::UserDefined),
      is_dynamic_entry: matches!(&self.entry_point, Some(e) if e.kind == EntryPointKind::DynamicImport),
      facade_module_id: self
        .entry_point
        .as_ref()
        .map(|entry_point| graph.modules[entry_point.id].expect_normal().pretty_path.to_string()),
      module_ids: self
        .modules
        .iter()
        .map(|id| graph.modules[*id].expect_normal().pretty_path.to_string())
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
