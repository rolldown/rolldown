use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::{Joiner, JoinerOptions};

use crate::bundler::{
  bitset::BitSet,
  graph::{graph::Graph, symbols::Symbols},
  module::{module::ModuleRenderContext, module_id::ModuleVec},
  options::{
    file_name_template::FileNameRenderOptions, normalized_output_options::NormalizedOutputOptions,
  },
};

use super::{ChunkId, ChunksVec};

#[derive(Debug, Default)]
pub struct Chunk {
  pub entry_module: Option<ModuleId>,
  pub modules: Vec<ModuleId>,
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Atom>,
  pub exports_str: Option<String>,
  pub bits: BitSet,
}

impl Chunk {
  pub fn new(
    name: Option<String>,
    entry_module: Option<ModuleId>,
    bits: BitSet,
    modules: Vec<ModuleId>,
  ) -> Self {
    Self {
      name,
      entry_module,
      bits,
      modules,
      ..Default::default()
    }
  }

  pub fn render_file_name(&mut self, output_options: &NormalizedOutputOptions) {
    self.file_name = Some(
      output_options
        .entry_file_names
        .render(FileNameRenderOptions {
          name: self.name.as_deref(),
        }),
    )
  }

  pub fn initialize_exports(&mut self, modules: &mut ModuleVec, symbols: &Symbols) {
    let entry = &mut modules[*self.modules.last().unwrap()];

    // export { };
    if !entry.expect_normal().resolved_exports.is_empty() {
      let mut resolved_exports = entry
        .expect_normal()
        .resolved_exports
        .iter()
        .collect::<Vec<_>>();
      resolved_exports.sort_by_key(|(name, _)| name.as_str());
      let mut exports_str = "export { ".to_string();
      exports_str.push_str(
        &resolved_exports
          .into_iter()
          .map(|(exported, refer)| {
            let final_name = self
              .canonical_names
              .get(&symbols.par_get_canonical_ref(refer.local_symbol))
              .cloned()
              .unwrap_or_else(|| panic!("not found {:?}", exported));
            if final_name == exported {
              format!("{}", final_name)
            } else {
              format!("{} as {}", final_name, exported,)
            }
          })
          .collect::<Vec<_>>()
          .join(", "),
      );
      exports_str.push_str(" };");
      self.exports_str = Some(exports_str);
    }
  }

  pub fn render(
    &self,
    graph: &Graph,
    module_to_chunk: &IndexVec<ModuleId, Option<ChunkId>>,
    chunks: &ChunksVec,
  ) -> anyhow::Result<String> {
    use rayon::prelude::*;
    let mut joiner = Joiner::with_options(JoinerOptions {
      separator: Some("\n".to_string()),
    });
    self
      .modules
      .par_iter()
      .copied()
      .map(|id| &graph.modules[id])
      .filter_map(|m| {
        m.render(ModuleRenderContext {
          canonical_names: &self.canonical_names,
          symbols: &graph.symbols,
          module_to_chunk,
          chunks,
        })
      })
      .collect::<Vec<_>>()
      .into_iter()
      .for_each(|item| {
        joiner.append(item);
      });
    if let Some(exports) = self.exports_str.clone() {
      joiner.append_raw(exports);
    }

    Ok(joiner.join())
  }
}

#[derive(Debug, Clone)]
pub struct ImportChunkMeta {
  pub chunk_id: ChunkId,
  // pub symbols: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ChunkMeta {
  pub imports: Vec<ImportChunkMeta>,
}

pub type CrossChunksMeta = IndexVec<ChunkId, ChunkMeta>;
