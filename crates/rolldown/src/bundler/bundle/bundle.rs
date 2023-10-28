use std::{borrow::Cow, hash::BuildHasherDefault};

use super::asset::Asset;
use crate::bundler::{
  bitset::BitSet,
  chunk::{
    chunk::{Chunk, CrossChunkImportItem},
    ChunkId, ChunksVec,
  },
  chunk_graph::ChunkGraph,
  graph::graph::Graph,
  module::Module,
  options::{
    normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};
use anyhow::Ok;
use index_vec::{index_vec, IndexVec};
use rolldown_common::{ImportKind, ModuleId, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};

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
      // Module imported dynamically will be considered as an entry,
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

  fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    // Determine which symbols belong to which chunk
    let mut chunk_meta_imports_vec =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut chunk_meta_exports_vec =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    for (chunk_id, chunk) in chunk_graph.chunks.iter_enumerated() {
      let chunk_meta_imports = &mut chunk_meta_imports_vec[chunk_id];

      for module_id in chunk.modules.iter().copied() {
        match &self.graph.modules[module_id] {
          Module::Normal(module) => {
            let linking_info = &self.graph.linking_infos[module_id];

            for stmt_info in module.stmt_infos.iter().chain(linking_info.facade_stmt_infos.iter()) {
              for declared in &stmt_info.declared_symbols {
                // TODO: pass debug_assert!(self.graph.symbols.get(*declared).chunk_id.is_none());
                // FIXME: I don't think this is correct, even though the assigned chunk_id is the same as the current chunk_id.
                // A declared symbol should only be processed once.
                debug_assert!(
                  self.graph.symbols.get(*declared).chunk_id.unwrap_or(chunk_id) == chunk_id
                );

                self.graph.symbols.get_mut(*declared).chunk_id = Some(chunk_id);
              }

              for referenced in &stmt_info.referenced_symbols {
                let canonical_ref = self.graph.symbols.get_canonical_ref(*referenced);
                chunk_meta_imports.insert(canonical_ref);
              }
            }
          }
          Module::External(_) => {
            // TODO: process external module
          }
        }
      }

      if let Some(entry_module) = chunk.entry_module {
        let entry_module = &self.graph.modules[entry_module];
        let entry_linking_info = &self.graph.linking_infos[entry_module.id()];
        for export_ref in entry_linking_info.resolved_exports.values() {
          let mut canonical_ref = self.graph.symbols.get_canonical_ref(*export_ref);
          let symbol = self.graph.symbols.get(canonical_ref);
          if let Some(ns_alias) = &symbol.namespace_alias {
            canonical_ref = ns_alias.namespace_ref;
          }
          chunk_meta_imports.insert(canonical_ref);
        }
      }
    }
    for (chunk_id, chunk) in chunk_graph.chunks.iter_mut_enumerated() {
      let chunk_meta_imports = &chunk_meta_imports_vec[chunk_id];
      for import_ref in chunk_meta_imports.iter().copied() {
        let import_symbol = self.graph.symbols.get(import_ref);
        let importee_chunk_id = import_symbol.chunk_id.unwrap_or_else(|| {
          panic!("symbol {import_ref:?} {import_symbol:?} is not assigned to any chunk")
        });
        if chunk_id != importee_chunk_id {
          chunk
            .imports_from_other_chunks
            .entry(importee_chunk_id)
            .or_default()
            .push(CrossChunkImportItem { import_ref, export_alias: None });
          chunk_meta_exports_vec[importee_chunk_id].insert(import_ref);
        }
      }

      if chunk.entry_module.is_none() {
        continue;
      }
      // If this is an entry point, make sure we import all chunks belonging to
      // this entry point, even if there are no imports. We need to make sure
      // these chunks are evaluated for their side effects too.
      // TODO: ensure chunks are evaluated for their side effects too.
    }
    // Generate cross-chunk exports. These must be computed before cross-chunk
    // imports because of export alias renaming, which must consider all export
    // aliases simultaneously to avoid collisions.
    let mut name_count = FxHashMap::default();
    for (chunk_id, chunk) in chunk_graph.chunks.iter_mut_enumerated() {
      for export in chunk_meta_exports_vec[chunk_id].iter().copied() {
        let original_name = self.graph.symbols.get_original_name(export);
        let count = name_count.entry(Cow::Borrowed(original_name)).or_insert(0u32);
        let alias = if *count == 0 {
          original_name.clone()
        } else {
          format!("{original_name}${count}").into()
        };
        chunk.exports_to_other_chunks.insert(export, alias.clone());
        *count += 1;
      }
    }
    for chunk_id in chunk_graph.chunks.indices() {
      for (importee_chunk_id, import_items) in
        &chunk_graph.chunks[chunk_id].imports_from_other_chunks
      {
        for item in import_items {
          if let Some(alias) =
            chunk_graph.chunks[*importee_chunk_id].exports_to_other_chunks.get(&item.import_ref)
          {
            // safety: no other mutable reference to `item` exists
            unsafe {
              let item = (item as *const CrossChunkImportItem).cast_mut();
              (*item).export_alias = Some(alias.clone());
            }
          }
        }
      }
    }
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

    // FIXME: should only do this while `ROLLDOWN_TEST=1`
    let _runtime_chunk_id = chunks.push(Chunk::new(
      Some("_rolldown_runtime".to_string()),
      None,
      BitSet::new(0),
      vec![self.graph.runtime.id],
    ));

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

    self.compute_cross_chunk_links(&mut chunk_graph);

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
