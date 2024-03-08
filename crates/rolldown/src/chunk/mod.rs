mod de_conflict;
pub mod render_chunk;
mod render_chunk_exports;
mod render_chunk_imports;
use index_vec::IndexVec;
use rolldown_common::ChunkId;

pub type ChunksVec = IndexVec<ChunkId, Chunk>;

use rolldown_common::{
  ChunkKind, ExternalModuleId, NamedImport, NormalModuleId, RenderedModule, Specifier, SymbolRef,
};
use rolldown_error::BuildError;
use rolldown_rstr::Rstr;
use rolldown_sourcemap::{collapse_sourcemaps, concat_sourcemaps, SourceMap};
use rustc_hash::FxHashMap;

use crate::utils::render_normal_module::render_normal_module;
use crate::{
  error::BatchedResult,
  FileNameTemplate, InputOptions,
  {
    chunk_graph::ChunkGraph, options::output_options::OutputOptions,
    stages::link_stage::LinkStageOutput, types::module_render_context::ModuleRenderContext,
    utils::bitset::BitSet,
  },
};

#[derive(Debug)]
pub struct CrossChunkImportItem {
  pub export_alias: Option<Specifier>,
  pub import_ref: SymbolRef,
}

#[derive(Debug, Default)]
pub struct Chunk {
  pub kind: ChunkKind,
  pub modules: Vec<NormalModuleId>,
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  pub bits: BitSet,
  pub imports_from_other_chunks: FxHashMap<ChunkId, Vec<CrossChunkImportItem>>,
  pub imports_from_external_modules: FxHashMap<ExternalModuleId, Vec<NamedImport>>,
  // meaningless if the chunk is an entrypoint
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Rstr>,
}

pub struct ChunkRenderReturn {
  pub code: String,
  pub map: Option<SourceMap>,
  pub rendered_modules: FxHashMap<String, RenderedModule>,
}

impl Chunk {
  pub fn new(
    name: Option<String>,
    bits: BitSet,
    modules: Vec<NormalModuleId>,
    kind: ChunkKind,
  ) -> Self {
    Self { modules, name, bits, kind, ..Self::default() }
  }

  pub fn file_name_template<'a>(
    &mut self,
    output_options: &'a OutputOptions,
  ) -> &'a FileNameTemplate {
    if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined) {
      &output_options.entry_file_names
    } else {
      &output_options.chunk_file_names
    }
  }

  #[allow(clippy::unnecessary_wraps, clippy::cast_possible_truncation)]
  pub fn render(
    &self,
    input_options: &InputOptions,
    graph: &LinkStageOutput,
    chunk_graph: &ChunkGraph,
    output_options: &OutputOptions,
  ) -> BatchedResult<ChunkRenderReturn> {
    use rayon::prelude::*;
    let mut rendered_modules = FxHashMap::default();
    let mut content_and_sourcemaps = vec![];

    content_and_sourcemaps
      .push((self.render_imports_for_esm(graph, chunk_graph).to_string(), None));

    self
      .modules
      .par_iter()
      .copied()
      .map(|id| &graph.module_table.normal_modules[id])
      .filter_map(|m| {
        let rendered_content = render_normal_module(
          m,
          &ModuleRenderContext {
            canonical_names: &self.canonical_names,
            graph,
            chunk_graph,
            input_options,
          },
          &graph.ast_table[m.id],
        );
        Some((
          m.resource_id.expect_file().to_string(),
          RenderedModule {
            original_length: m.source.len().try_into().unwrap(),
            rendered_length: rendered_content
              .as_ref()
              .map(|c| c.code.len() as u32)
              .unwrap_or_default(),
          },
          rendered_content,
          if output_options.sourcemap.is_hidden() {
            None
          } else {
            // TODO add oxc codegen sourcemap to sourcemap chain
            Some(collapse_sourcemaps(m.sourcemap_chain.clone()))
          },
        ))
      })
      .collect::<Vec<_>>()
      .into_iter()
      .try_for_each(
        |(module_path, rendered_module, rendered_content, map)| -> Result<(), BuildError> {
          if let Some(rendered_content) = rendered_content {
            content_and_sourcemaps.push((
              rendered_content.code.to_string(),
              match map {
                None => None,
                Some(v) => v?,
              },
            ));
          }
          rendered_modules.insert(module_path, rendered_module);
          Ok(())
        },
      )?;

    if let Some(exports) = self.render_exports(graph, output_options) {
      content_and_sourcemaps.push((exports.to_string(), None));
    }

    if output_options.sourcemap.is_hidden() {
      return Ok(ChunkRenderReturn {
        code: content_and_sourcemaps.into_iter().map(|(c, _)| c).collect::<Vec<_>>().join("\n"),
        map: None,
        rendered_modules,
      });
    }

    let (content, map) = concat_sourcemaps(&content_and_sourcemaps)?;

    Ok(ChunkRenderReturn { code: content, map: Some(map), rendered_modules })
  }
}
