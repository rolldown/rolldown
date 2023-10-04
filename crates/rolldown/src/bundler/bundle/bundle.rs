use super::asset::Asset;
use crate::bundler::{
  chunk::{
    chunk::{Chunk, ChunkMeta, CrossChunksMeta, ImportChunkMeta},
    ChunksVec,
  },
  graph::graph::Graph,
  module::module::ModuleFinalizeContext,
  options::{
    normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};
use anyhow::Ok;
use fixedbitset::FixedBitSet;
use index_vec::IndexVec;
use rolldown_common::ModuleId;
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
    modules_entry_bit: &mut IndexVec<ModuleId, FixedBitSet>,
  ) {
    if modules_entry_bit[module_id].count_ones(index..index + 1) > 0 {
      return;
    }
    modules_entry_bit[module_id].insert_range(index..index + 1);
    self.graph.modules[module_id]
      .expect_normal()
      .import_records
      .iter()
      .for_each(|i| self.mark_modules_entry_bit(i.resolved_module, index, modules_entry_bit));
  }

  pub fn generate_chunks(&self) -> ChunksVec {
    let mut modules_entry_bit =
      IndexVec::from_vec(vec![
        FixedBitSet::with_capacity(self.graph.entries.len());
        self.graph.modules.len()
      ]);

    self
      .graph
      .entries
      .iter()
      .enumerate()
      .for_each(|(i, (_, entry))| {
        self.mark_modules_entry_bit(*entry, i, &mut modules_entry_bit);
      });

    let mut chunk_map = self
      .graph
      .entries
      .iter()
      .enumerate()
      .map(|(i, (name, _))| {
        let mut bits = FixedBitSet::with_capacity(self.graph.entries.len());
        bits.insert_range(i..i + 1);
        (bits.clone(), Chunk::new(name.clone(), true, bits, vec![]))
      })
      .collect::<FxHashMap<FixedBitSet, Chunk>>();

    self.graph.modules.iter().for_each(|module| {
      let bit = &modules_entry_bit[module.id()];
      if !bit.is_empty() {
        if let Some(chunk) = chunk_map.get_mut(bit) {
          chunk.modules.push(module.id());
        } else {
          // TODO share chunk name
          let len = chunk_map.len();
          chunk_map.insert(
            bit.clone(),
            Chunk::new(Some(len.to_string()), false, bit.clone(), vec![module.id()]),
          );
        }
      }
    });

    let chunks = chunk_map
      .into_iter()
      .map(|(_, mut chunk)| {
        chunk
          .modules
          .sort_by_key(|id| self.graph.modules[*id].exec_order());
        chunk
      })
      .collect::<ChunksVec>();

    chunks
  }

  pub fn generate_cross_chunks_meta(&mut self, chunks: &ChunksVec) -> CrossChunksMeta {
    let mut chunks_meta: CrossChunksMeta =
      IndexVec::from_vec(vec![ChunkMeta::default(); chunks.len()]);
    chunks.iter().enumerate().for_each(|(chunk_id, chunk)| {
      if chunk.is_entry {
        chunks
          .iter()
          .enumerate()
          .for_each(|(other_chunk_id, other_chunk)| {
            if other_chunk_id != chunk_id && other_chunk.bits.is_superset(&chunk.bits) {
              chunks_meta[chunk_id].imports.push(ImportChunkMeta {
                chunk_id: other_chunk_id.into(),
              });
            }
          });
      }
    });
    chunks_meta
  }

  pub fn generate(
    &mut self,
    input_options: &'a NormalizedInputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    use rayon::prelude::*;
    let mut chunks = self.generate_chunks();
    let generate_cross_chunks_meta = self.generate_cross_chunks_meta(&chunks);
    chunks
      .iter_mut()
      .par_bridge()
      .for_each(|chunk| chunk.render_file_name(self.output_options));

    chunks.iter_mut().par_bridge().for_each(|chunk| {
      chunk.de_conflict(self.graph);
    });

    chunks.iter_mut().for_each(|chunk| {
      if chunk.is_entry {
        chunk.initialize_exports(&mut self.graph.modules, &self.graph.symbols);
      }
    });

    self
      .graph
      .modules
      .iter_mut()
      .par_bridge()
      .for_each(|module| {
        module.finalize(ModuleFinalizeContext {
          canonical_names: &chunks[0].canonical_names,
          symbols: &self.graph.symbols,
        });
      });

    let assets = chunks
      .iter()
      .enumerate()
      .map(|(chunk_id, c)| {
        let imports = c.generate_cross_chunk_links(&generate_cross_chunks_meta[chunk_id], &chunks);
        let content = c.render(self.graph, imports, input_options).unwrap();

        Asset {
          file_name: c.file_name.clone().unwrap(),
          content,
        }
      })
      .collect::<Vec<_>>();

    Ok(assets)
  }
}
