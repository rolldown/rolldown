use rolldown_common::{
  ExternalModuleId, NamedImport, NormalModuleId, RenderedModule, Specifier, SymbolRef,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcCompiler;
use rolldown_rstr::Rstr;
use rolldown_sourcemap::{collapse_sourcemaps, concat_sourcemaps, SourceMap};
use rustc_hash::FxHashMap;

use crate::{
  error::BatchedResult,
  FileNameTemplate, InputOptions,
  {
    chunk_graph::ChunkGraph, options::output_options::OutputOptions,
    stages::link_stage::LinkStageOutput, utils::bitset::BitSet,
  },
};

use super::ChunkId;

#[derive(Debug)]
pub struct CrossChunkImportItem {
  pub export_alias: Option<Specifier>,
  pub import_ref: SymbolRef,
}

#[derive(Debug)]
pub enum ChunkKind {
  EntryPoint { is_user_defined: bool, bit: u32, module: NormalModuleId },
  Common,
}

impl Default for ChunkKind {
  fn default() -> Self {
    Self::Common
  }
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

  #[allow(clippy::unnecessary_wraps, clippy::cast_possible_truncation, clippy::type_complexity)]
  pub fn render(
    &self,
    _input_options: &InputOptions,
    graph: &LinkStageOutput,
    chunk_graph: &ChunkGraph,
    output_options: &OutputOptions,
  ) -> BatchedResult<((String, Option<SourceMap>), FxHashMap<String, RenderedModule>)> {
    use rayon::prelude::*;
    let mut rendered_modules = FxHashMap::default();
    let mut content_and_sourcemaps = vec![];

    content_and_sourcemaps.push((
      self.render_imports_for_esm(graph, chunk_graph).to_string(),
      None,
      None,
    ));

    self
      .modules
      .par_iter()
      .copied()
      .map(|id| &graph.module_table.normal_modules[id])
      .filter_map(|m| {
        let rendered_content = OxcCompiler::print(&graph.ast_table[m.id]);
        Some((
          m.resource_id.expect_file().to_string(),
          RenderedModule {
            original_length: m.source.len().try_into().unwrap(),
            rendered_length: rendered_content.len() as u32,
          },
          rendered_content,
          if output_options.sourcemap.is_hidden() {
            None
          } else {
            // TODO add oxc codegen sourcemap to sourcemap chain
            Some(collapse_sourcemaps(m.sourcemap_chain.clone()))
          },
          &m.pretty_path,
        ))
      })
      .collect::<Vec<_>>()
      .into_iter()
      .try_for_each(
        |(module_path, rendered_module, rendered_content, map, pretty_path)| -> Result<(), BuildError> {
          content_and_sourcemaps.push((
            rendered_content.to_string(),
            match map {
              None => None,
              Some(v) => v?,
            },
            Some(format!("// {pretty_path}\n")),
          ));
          rendered_modules.insert(module_path, rendered_module);
          Ok(())
        },
      )?;

    if let Some(exports) = self.render_exports(graph, output_options) {
      content_and_sourcemaps.push((exports.to_string(), None, None));
    }

    if output_options.sourcemap.is_hidden() {
      return Ok((
        (
          content_and_sourcemaps
            .into_iter()
            .map(|(content, _, extra_start_content)| match extra_start_content {
              Some(extra_start_content) => format!("{extra_start_content}{content}"),
              None => content,
            })
            .collect::<Vec<_>>()
            .join("\n"),
          None,
        ),
        rendered_modules,
      ));
    }

    let (content, map) = concat_sourcemaps(&content_and_sourcemaps)?;
    Ok(((content, Some(map)), rendered_modules))
  }
}
