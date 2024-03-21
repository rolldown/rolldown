// cSpell:disable
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
use rolldown_sourcemap::{
  collapse_sourcemaps, ConcatSource, RawSource, SourceMap, SourceMapSource,
};
use rolldown_utils::BitSet;
use rustc_hash::FxHashMap;

use crate::options::normalized_input_options::NormalizedInputOptions;
use crate::options::normalized_output_options::NormalizedOutputOptions;
use crate::utils::render_normal_module::render_normal_module;
use crate::{
  error::BatchedResult,
  FileNameTemplate,
  {
    chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput,
    types::module_render_context::ModuleRenderContext,
  },
};

#[derive(Debug, Clone)]
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
    output_options: &'a NormalizedOutputOptions,
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
    input_options: &NormalizedInputOptions,
    graph: &LinkStageOutput,
    chunk_graph: &ChunkGraph,
    output_options: &NormalizedOutputOptions,
  ) -> BatchedResult<ChunkRenderReturn> {
    use rayon::prelude::*;
    let mut rendered_modules = FxHashMap::default();
    let mut concat_source = ConcatSource::default();

    concat_source
      .add_source(Box::new(RawSource::new(self.render_imports_for_esm(graph, chunk_graph))));

    self
      .modules
      .par_iter()
      .copied()
      .map(|id| &graph.module_table.normal_modules[id])
      .filter_map(|m| {
        let rendered_output = render_normal_module(
          &ModuleRenderContext {
            canonical_names: &self.canonical_names,
            graph,
            chunk_graph,
            input_options,
          },
          &graph.ast_table[m.id],
          if output_options.sourcemap.is_hidden() {
            None
          } else {
            Some(
              m.resource_id
                .expect_file()
                .relative_path(&input_options.cwd)
                .to_string_lossy()
                .to_string(),
            )
          },
        );
        Some((
          m.resource_id.expect_file().to_string(),
          &m.pretty_path,
          RenderedModule { code: None },
          rendered_output.as_ref().map(|v| v.source_text.to_string()),
          if output_options.sourcemap.is_hidden() {
            None
          } else {
            let mut sourcemap_chain = m.sourcemap_chain.iter().collect::<Vec<_>>();
            if let Some(Some(sourcemap)) = rendered_output.as_ref().map(|x| x.source_map.as_ref()) {
              sourcemap_chain.push(sourcemap);
            }
            Some(collapse_sourcemaps(sourcemap_chain))
          },
        ))
      })
      .collect::<Vec<_>>()
      .into_iter()
      .try_for_each(
        |(module_path, module_pretty_path, rendered_module, rendered_content, map)| -> Result<(), BuildError> {
          if let Some(rendered_content) = rendered_content {
            concat_source.add_source(Box::new(RawSource::new(format!("// {module_pretty_path}"))));
            if let Some(map) = match map {
              None => None,
              Some(v) => v?,
            } {
              concat_source.add_source(Box::new(SourceMapSource::new(rendered_content, map)));
            } else {
              concat_source.add_source(Box::new(RawSource::new(rendered_content)));
            }
          }
          rendered_modules.insert(module_path, rendered_module);
          Ok(())
        },
      )?;
    // add banner
    if let Some(banner) = output_options.banner {
      concat_source.prepend_source(Box::new(RawSource::new(banner)))
    }
    if let Some(exports) = self.render_exports(graph, output_options) {
      concat_source.add_source(Box::new(RawSource::new(exports)));
    }

    let (content, map) = concat_source.content_and_sourcemap();

    Ok(ChunkRenderReturn { code: content, map, rendered_modules })
  }
}
