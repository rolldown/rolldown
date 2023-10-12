use super::asset::Asset;
use crate::bundler::{
  bitset::BitSet,
  chunk::{
    chunk::{Chunk, CrossChunksMeta},
    ChunkId, ChunksVec,
  },
  graph::graph::Graph,
  module::module::Module,
  module_loader::ModuleLoader,
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
  module_loader: &'a mut ModuleLoader<'a>,
  output_options: &'a NormalizedOutputOptions,
}

impl<'a> Bundle<'a> {
  pub fn new(
    graph: &'a mut Graph,
    module_loader: &'a mut ModuleLoader<'a>,
    output_options: &'a NormalizedOutputOptions,
  ) -> Self {
    Self {
      graph,
      module_loader,
      output_options,
    }
  }

  pub fn mark_modules_entry_bit(
    &self,
    module_id: ModuleId,
    bit: u32,
    enforce_only_bit: bool,
    modules_entry_bit: &mut IndexVec<ModuleId, BitSet>,
  ) {
    if modules_entry_bit[module_id].has_bit(bit) {
      return;
    }
    if enforce_only_bit {
      modules_entry_bit[module_id].clear();
    }
    modules_entry_bit[module_id].set_bit(bit);
    if let Module::Normal(m) = &self.graph.modules[module_id] {
      m.import_records.iter().for_each(|i| {
        // because dynamic import is already as entry, so here ignore it
        if i.kind != ImportKind::DynamicImport {
          self.mark_modules_entry_bit(i.resolved_module, bit, false, modules_entry_bit)
        }
      });
    }
  }

  pub async fn normalize_manual_chunks(
    &self,
  ) -> anyhow::Result<Option<FxHashMap<String, Vec<ModuleId>>>> {
    if let Some(manual_chunks) = &self.output_options.manual_chunks {
      let mut normalize_manual_chunks = FxHashMap::default();
      for (name, module_names) in manual_chunks.iter() {
        let modules = self
          .module_loader
          .resolve_manual_chunk_modules(module_names)
          .await?;
        normalize_manual_chunks.insert(name.clone(), modules);
      }
      Ok(Some(normalize_manual_chunks))
    } else {
      Ok(None)
    }
  }

  pub async fn generate_chunks(
    &self,
  ) -> anyhow::Result<(ChunksVec, IndexVec<ModuleId, Option<ChunkId>>)> {
    // Create chunks for
    // - entries
    // - dynamic entries, it is already add to entries at module_loader.rs
    // - manual_chunks options
    let manual_chunks = self.normalize_manual_chunks().await?;

    let mut chunks = FxHashMap::default();

    let manual_chunks_len = manual_chunks.as_ref().map(|s| s.len()).unwrap_or_default();
    let initial_chunks_len = manual_chunks_len + self.graph.entries.len();

    chunks.shrink_to(initial_chunks_len);

    let mut module_to_bits = index_vec::index_vec![
      BitSet::new(initial_chunks_len as u32);
      self.graph.modules.len()
    ];

    for (i, (name, module_id)) in self.graph.entries.iter().enumerate() {
      let count: u32 = i as u32;
      let mut entry_bits = BitSet::new(initial_chunks_len as u32);
      entry_bits.set_bit(count);

      // Mark module corresponding entry bit
      self.mark_modules_entry_bit(*module_id, count, false, &mut module_to_bits);

      let c = Chunk::new(name.clone(), Some(*module_id), entry_bits.clone(), vec![]);
      chunks.insert(entry_bits, c);
    }

    if let Some(manual_chunks) = manual_chunks {
      for (i, (name, modules)) in manual_chunks.into_iter().enumerate() {
        let count: u32 = (i + self.graph.entries.len()) as u32;
        let mut entry_bits = BitSet::new(initial_chunks_len as u32);
        entry_bits.set_bit(count);

        // Mark module corresponding entry bit
        // Note: manual chunks will enforce module to a chunk
        modules
          .iter()
          .for_each(|m| self.mark_modules_entry_bit(*m, count, true, &mut module_to_bits));

        let c = Chunk::new(Some(name), None, entry_bits.clone(), vec![]);
        chunks.insert(entry_bits, c);
      }
    }

    // Connect module to already create chunks, else create a new chunk to connect
    self
      .graph
      .modules
      .iter()
      .enumerate()
      // TODO avoid generate runtime module
      .skip_while(|(module_id, _)| module_id.eq(&self.graph.runtime.id)) // TODO avoid generate runtime module
      .for_each(|(_, module)| {
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

    let chunks = chunks
      .into_values()
      .map(|mut chunk| {
        chunk
          .modules
          .sort_by_key(|id| self.graph.modules[*id].exec_order());
        chunk
      })
      .collect::<ChunksVec>();

    let mut module_to_chunk: IndexVec<ModuleId, Option<ChunkId>> = index_vec::index_vec![
      None;
      self.graph.modules.len()
    ];

    // perf: this process could be done with computing chunks together
    for (i, chunk) in chunks.iter_enumerated() {
      for module_id in &chunk.modules {
        module_to_chunk[*module_id] = Some(i);
      }
    }

    Ok((chunks, module_to_chunk))
  }

  pub fn generate_cross_chunks_meta(&mut self, _chunks: &ChunksVec) -> CrossChunksMeta {
    // TODO: cross chunk imports
    Default::default()
  }

  pub async fn generate(
    &mut self,
    _input_options: &'a NormalizedInputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    use rayon::prelude::*;
    let (mut chunks, module_to_chunk) = self.generate_chunks().await?;
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

    let assets = chunks
      .iter()
      .enumerate()
      .map(|(_chunk_id, c)| {
        let content = c.render(self.graph, &module_to_chunk, &chunks).unwrap();

        Asset {
          file_name: c.file_name.clone().unwrap(),
          content,
        }
      })
      .collect::<Vec<_>>();

    Ok(assets)
  }
}
