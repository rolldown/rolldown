use std::hash::BuildHasherDefault;

use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{Chunk, ChunkId, ChunkKind, ImportKind, ModuleId, NormalModuleId};
use rolldown_utils::{rustc_hash::FxHashMapExt, BitSet};
use rustc_hash::FxHashMap;

use crate::{chunk_graph::ChunkGraph, type_alias::IndexChunks, utils::is_in_rust_test_mode};

use super::GenerateStage;

impl<'a> GenerateStage<'a> {
  fn determine_reachable_modules_for_entry(
    &self,
    module_id: NormalModuleId,
    entry_index: u32,
    module_to_bits: &mut IndexVec<NormalModuleId, BitSet>,
  ) {
    let module = &self.link_output.module_table.normal_modules[module_id];

    if !module.is_included {
      return;
    }

    if module_to_bits[module_id].has_bit(entry_index) {
      return;
    }
    module_to_bits[module_id].set_bit(entry_index);

    module.import_records.iter().for_each(|rec| {
      if let ModuleId::Normal(importee_id) = rec.resolved_module {
        // Module imported dynamically will be considered as an entry,
        // so we don't need to include it in this chunk
        if !matches!(rec.kind, ImportKind::DynamicImport) {
          self.determine_reachable_modules_for_entry(importee_id, entry_index, module_to_bits);
        }
      }
    });

    module.stmt_infos.iter().for_each(|stmt_info| {
      if !stmt_info.is_included {
        return;
      }
      stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
        let canonical_ref = self.link_output.symbols.par_canonical_ref_for(*symbol_ref);
        self.determine_reachable_modules_for_entry(
          canonical_ref.owner,
          entry_index,
          module_to_bits,
        );
      });
    });
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn generate_chunks(&self) -> ChunkGraph {
    let entries_len: u32 =
      self.link_output.entries.len().try_into().expect("Too many entries, u32 overflowed.");
    // If we are in test environment, to make the runtime module always fall into a standalone chunk,
    // we create a facade entry point for it.
    let entries_len = if is_in_rust_test_mode() { entries_len + 1 } else { entries_len };

    let mut module_to_bits = oxc_index::index_vec![BitSet::new(entries_len); self.link_output.module_table.normal_modules.len()];
    let mut bits_to_chunk = FxHashMap::with_capacity_and_hasher(
      self.link_output.entries.len(),
      BuildHasherDefault::default(),
    );
    let mut chunks = IndexChunks::with_capacity(self.link_output.entries.len());
    let mut user_defined_entry_chunk_ids: Vec<ChunkId> = Vec::new();
    let mut entry_module_to_entry_chunk: FxHashMap<NormalModuleId, ChunkId> =
      FxHashMap::with_capacity(self.link_output.entries.len());
    // Create chunk for each static and dynamic entry
    for (entry_index, entry_point) in self.link_output.entries.iter().enumerate() {
      let count: u32 = entry_index.try_into().expect("Too many entries, u32 overflowed.");
      let mut bits = BitSet::new(entries_len);
      bits.set_bit(count);
      let module = &self.link_output.module_table.normal_modules[entry_point.id];
      let chunk = chunks.push(Chunk::new(
        entry_point.name.clone(),
        bits.clone(),
        vec![],
        ChunkKind::EntryPoint {
          is_user_defined: module.is_user_defined_entry,
          bit: count,
          module: entry_point.id,
        },
      ));
      bits_to_chunk.insert(bits, chunk);
      entry_module_to_entry_chunk.insert(entry_point.id, chunk);
      if entry_point.kind.is_user_defined() {
        user_defined_entry_chunk_ids.push(chunk);
      }
    }

    if is_in_rust_test_mode() {
      self.determine_reachable_modules_for_entry(
        self.link_output.runtime.id(),
        entries_len - 1,
        &mut module_to_bits,
      );
    }

    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    self.link_output.entries.iter().enumerate().for_each(|(i, entry_point)| {
      self.determine_reachable_modules_for_entry(
        entry_point.id,
        i.try_into().expect("Too many entries, u32 overflowed."),
        &mut module_to_bits,
      );
    });

    let mut module_to_chunk: IndexVec<NormalModuleId, Option<ChunkId>> = oxc_index::index_vec![
      None;
      self.link_output.module_table.normal_modules.len()
    ];

    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for normal_module in &self.link_output.module_table.normal_modules {
      if !normal_module.is_included {
        continue;
      }

      let bits = &module_to_bits[normal_module.id];
      debug_assert!(
        !bits.is_empty(),
        "Empty bits means the module is not reachable, so it should bail out with `is_included: false`"
      );
      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunks[chunk_id].modules.push(normal_module.id);
        module_to_chunk[normal_module.id] = Some(chunk_id);
      } else {
        let chunk = Chunk::new(None, bits.clone(), vec![normal_module.id], ChunkKind::Common);
        let chunk_id = chunks.push(chunk);
        module_to_chunk[normal_module.id] = Some(chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }

    // Sort modules in each chunk by execution order
    chunks.iter_mut().for_each(|chunk| {
      chunk.modules.sort_by_key(|module_id| {
        self.link_output.module_table.normal_modules[*module_id].exec_order
      });
    });

    let sorted_chunk_ids =
      chunks.indices().sorted_by_key(|id| &chunks[*id].bits).collect::<Vec<_>>();

    ChunkGraph {
      chunks,
      sorted_chunk_ids,
      module_to_chunk,
      entry_module_to_entry_chunk,
      user_defined_entry_chunk_ids,
    }
  }
}
