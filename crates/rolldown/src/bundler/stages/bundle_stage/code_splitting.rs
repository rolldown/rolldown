use std::hash::BuildHasherDefault;

use index_vec::IndexVec;
use rolldown_common::{ImportKind, ModuleId};
use rustc_hash::FxHashMap;

use crate::bundler::{
  chunk::{
    chunk::{Chunk, ChunkKind},
    ChunkId, ChunksVec,
  },
  chunk_graph::ChunkGraph,
  module::Module,
  utils::bitset::BitSet,
};

use super::BundleStage;

impl<'a> BundleStage<'a> {
  fn determine_reachable_modules_for_entry(
    &self,
    module_id: ModuleId,
    entry_index: u32,
    module_to_bits: &mut IndexVec<ModuleId, BitSet>,
  ) {
    let Module::Normal(module) = &self.link_output.modules[module_id] else { return };

    if !module.is_included {
      return;
    }

    if module_to_bits[module_id].has_bit(entry_index) {
      return;
    }
    module_to_bits[module_id].set_bit(entry_index);

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

  pub fn generate_chunks(&self) -> ChunkGraph {
    let entries_len: u32 =
      self.link_output.entries.len().try_into().expect("Too many entries, u32 overflowed.");
    let is_rolldown_test = std::env::var("ROLLDOWN_TEST").is_ok();
    // If we are in test environment, to make the runtime module always fall into a standalone chunk,
    // we create a facade entry point for it.
    let entries_len = if is_rolldown_test { entries_len + 1 } else { entries_len };

    let mut module_to_bits =
      index_vec::index_vec![BitSet::new(entries_len); self.link_output.modules.len()];
    let mut bits_to_chunk = FxHashMap::with_capacity_and_hasher(
      self.link_output.entries.len(),
      BuildHasherDefault::default(),
    );
    let mut chunks = ChunksVec::with_capacity(self.link_output.entries.len());

    // Create chunk for each static and dynamic entry
    for (entry_index, entry_point) in self.link_output.entries.iter().enumerate() {
      let count: u32 = entry_index.try_into().expect("Too many entries, u32 overflowed.");
      let mut bits = BitSet::new(entries_len);
      bits.set_bit(count);
      let Module::Normal(module) = &self.link_output.modules[entry_point.id] else {
        unreachable!("Entry point should always be a normal module")
      };
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
    }

    if is_rolldown_test {
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

    let mut module_to_chunk: IndexVec<ModuleId, Option<ChunkId>> = index_vec::index_vec![
      None;
      self.link_output.modules.len()
    ];

    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for module in &self.link_output.modules {
      let Module::Normal(normal_module) = module else {
        continue;
      };

      if !normal_module.is_included {
        continue;
      }

      let bits = &module_to_bits[module.id()];
      debug_assert!(
        !bits.is_empty(),
        "Empty bits means the module is not reachable, so it should bail out with `is_included: false`"
      );
      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunks[chunk_id].modules.push(module.id());
        module_to_chunk[module.id()] = Some(chunk_id);
      } else {
        let chunk = Chunk::new(None, bits.clone(), vec![module.id()], ChunkKind::Common);
        let chunk_id = chunks.push(chunk);
        module_to_chunk[module.id()] = Some(chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }

    // Sort modules in each chunk by execution order
    chunks.iter_mut().for_each(|chunk| {
      chunk.modules.sort_by_key(|module_id| self.link_output.modules[*module_id].exec_order());
    });

    tracing::trace!("Generated chunks: {:#?}", chunks);

    ChunkGraph { chunks, module_to_chunk }
  }
}
