use std::hash::BuildHasherDefault;

use super::asset::Asset;
use crate::bundler::{
  bitset::BitSet,
  chunk::{chunk::Chunk, chunk_graph::ChunkGraph, ChunkId, ChunksVec},
  graph::graph::Graph,
  module::module::Module,
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
    Self { graph, output_options }
  }

  fn determine_reachable_modules_for_entry(
    &self,
    module_id: ModuleId,
    entry_index: u32,
    module_to_bits: &mut IndexVec<ModuleId, BitSet>,
  ) {
    if module_to_bits[module_id].has_bit(entry_index) {
      return;
    }
    module_to_bits[module_id].set_bit(entry_index);
    let Module::Normal(module) = &self.graph.modules[module_id] else { return };
    module.import_records.iter().for_each(|rec| {
      // Module imported dynamically will be considered as a entry,
      // so we don't need to include it in this chunk
      if rec.kind != ImportKind::DynamicImport {
        self.determine_reachable_modules_for_entry(
          rec.resolved_module,
          entry_index,
          module_to_bits,
        );
      }
    });
  }

  fn generate_chunks(&self) -> ChunkGraph {
    let entries_len: u32 = self.graph.entries.len().try_into().unwrap();

    let mut module_to_bits = index_vec::index_vec![
      BitSet::new(entries_len);
      self.graph.modules.len()
    ];
    let mut bits_to_chunk =
      FxHashMap::with_capacity_and_hasher(self.graph.entries.len(), BuildHasherDefault::default());
    let mut chunks = ChunksVec::with_capacity(self.graph.entries.len());

    // Create chunk for each static and dynamic entry
    for (entry_index, (name, module_id)) in self.graph.entries.iter().enumerate() {
      let count: u32 = u32::try_from(entry_index).unwrap();
      let mut bits = BitSet::new(entries_len);
      bits.set_bit(count);
      let chunk = chunks.push(Chunk::new(name.clone(), Some(*module_id), bits.clone(), vec![]));
      bits_to_chunk.insert(bits, chunk);
    }

    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    self.graph.entries.iter().enumerate().for_each(|(i, (_, entry))| {
      self.determine_reachable_modules_for_entry(
        *entry,
        i.try_into().unwrap(),
        &mut module_to_bits,
      );
    });

    let mut module_to_chunk: IndexVec<ModuleId, Option<ChunkId>> = index_vec::index_vec![
      None;
      self.graph.modules.len()
    ];

    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for module in &self.graph.modules {
      if module.id() == self.graph.runtime.id {
        // TODO: render runtime module
        continue;
      }
      let bits = &module_to_bits[module.id()];
      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunks[chunk_id].modules.push(module.id());
        module_to_chunk[module.id()] = Some(chunk_id);
      } else {
        let len = bits_to_chunk.len();
        // FIXME: https://github.com/rolldown-rs/rolldown/issues/49
        let chunk = Chunk::new(Some(len.to_string()), None, bits.clone(), vec![module.id()]);
        let chunk_id = chunks.push(chunk);
        module_to_chunk[module.id()] = Some(chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }

    // Sort modules in each chunk by execution order
    chunks.iter_mut().for_each(|chunk| {
      chunk.modules.sort_by_key(|module_id| self.graph.modules[*module_id].exec_order());
    });

    ChunkGraph { chunks, module_to_chunk }
  }

  pub fn generate(
    &mut self,
    _input_options: &'a NormalizedInputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    use rayon::prelude::*;
    let mut chunk_graph = self.generate_chunks();

    chunk_graph
      .chunks
      .iter_mut()
      .par_bridge()
      .for_each(|chunk| chunk.render_file_name(self.output_options));

    chunk_graph.chunks.iter_mut().par_bridge().for_each(|chunk| {
      chunk.de_conflict(self.graph);
    });

    chunk_graph.chunks.iter_mut().for_each(|chunk| {
      if chunk.entry_module.is_some() {
        chunk.initialize_exports(&self.graph.linker_modules, &self.graph.symbols);
      }
    });

    let assets = chunk_graph
      .chunks
      .iter()
      .enumerate()
      .map(|(_chunk_id, c)| {
        let content = c.render(self.graph, &chunk_graph).unwrap();

        Asset { file_name: c.file_name.clone().unwrap(), content }
      })
      .collect::<Vec<_>>();

    Ok(assets)
  }
}
