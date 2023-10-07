use super::asset::Asset;
use crate::bundler::{
  bitset::BitSet,
  chunk::{
    chunk::{Chunk, CrossChunksMeta},
    ChunksVec,
  },
  graph::graph::Graph,
  module::module::{Module, ModuleRenderContext},
  options::{
    normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};
use anyhow::Ok;
use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId};
use rustc_hash::FxHashMap;

pub struct Bundle<'a> {
  graph: &'a mut Graph,
  output_options: &'a NormalizedOutputOptions,
}

impl<'a> Bundle<'a> {
  pub fn new(graph: &'a mut Graph, output_options: &'a NormalizedOutputOptions) -> Self {
    Self {
      graph,
      output_options,
    }
  }

  pub fn mark_modules_entry_bit(
    &self,
    module_id: ModuleId,
    index: usize,
    modules_entry_bit: &mut IndexVec<ModuleId, BitSet>,
  ) {
    if modules_entry_bit[module_id].has_bit(index as u32) {
      return;
    }
    modules_entry_bit[module_id].set_bit(index as u32);
    if let Module::Normal(m) = &self.graph.modules[module_id] {
      m.import_records.iter().for_each(|i| {
        // because dynamic import is already as entry, so here ignore it
        if i.kind != ImportKind::DynamicImport {
          self.mark_modules_entry_bit(i.resolved_module, index, modules_entry_bit)
        }
      });
    }
  }

  pub fn generate_chunks(&self) -> ChunksVec {
    let mut module_to_bits = index_vec::index_vec![
      BitSet::new(self.graph.entries.len().try_into().unwrap());
      self.graph.modules.len()
    ];

    let mut chunks = FxHashMap::default();
    chunks.shrink_to(self.graph.entries.len());

    for (i, (name, module_id)) in self.graph.entries.iter().enumerate() {
      let count: u32 = i as u32;
      let mut entry_bits = BitSet::new(self.graph.entries.len() as u32);
      entry_bits.set_bit(count);
      let c = Chunk::new(name.clone(), Some(*module_id), entry_bits.clone(), vec![]);
      chunks.insert(entry_bits, c);
    }

    self
      .graph
      .entries
      .iter()
      .enumerate()
      .for_each(|(i, (_, entry))| {
        self.mark_modules_entry_bit(*entry, i, &mut module_to_bits);
      });

    self.graph.modules.iter().for_each(|module| {
      let bits = &module_to_bits[module.id()];
      if let Some(chunk) = chunks.get_mut(bits) {
        chunk.modules.push(module.id());
      } else {
        // TODO share chunk name
        let len = chunks.len();
        chunks.insert(
          bits.clone(),
          Chunk::new(Some(len.to_string()), None, bits.clone(), vec![module.id()]),
        );
      }
    });

    chunks
      .into_values()
      .map(|mut chunk| {
        chunk
          .modules
          .sort_by_key(|id| self.graph.modules[*id].exec_order());
        chunk
      })
      .collect::<ChunksVec>()
  }

  pub fn generate_cross_chunks_meta(&mut self, _chunks: &ChunksVec) -> CrossChunksMeta {
    // TODO: cross chunk imports
    Default::default()
  }

  pub fn generate(
    &mut self,
    input_options: &'a NormalizedInputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    use rayon::prelude::*;
    let mut chunks = self.generate_chunks();
    let _generate_cross_chunks_meta = self.generate_cross_chunks_meta(&chunks);
    chunks
      .iter_mut()
      .par_bridge()
      .for_each(|chunk| chunk.render_file_name(self.output_options));

    chunks.iter_mut().par_bridge().for_each(|chunk| {
      chunk.de_conflict(self.graph);
    });

    let mut entries_chunk_final_names = FxHashMap::default();
    entries_chunk_final_names.shrink_to(self.graph.entries.len());

    chunks.iter_mut().for_each(|chunk| {
      if let Some(module_id) = chunk.entry_module {
        entries_chunk_final_names.insert(module_id, chunk.file_name.clone().unwrap());
        chunk.initialize_exports(&mut self.graph.modules, &self.graph.symbols);
      }
    });

    self
      .graph
      .modules
      .iter_mut()
      .par_bridge()
      .for_each(|module| {
        module.render(ModuleRenderContext {
          canonical_names: &chunks[0].canonical_names,
          symbols: &self.graph.symbols,
          entries_chunk_final_names: &entries_chunk_final_names,
        });
      });

    let assets = chunks
      .iter()
      .enumerate()
      .map(|(_chunk_id, c)| {
        let content = c.render(self.graph, input_options).unwrap();

        Asset {
          file_name: c.file_name.clone().unwrap(),
          content,
        }
      })
      .collect::<Vec<_>>();

    Ok(assets)
  }
}
