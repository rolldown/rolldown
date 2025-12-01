use rolldown_common::{ChunkIdx, ChunkKind, ChunkMeta, ModuleIdx, ModuleTable};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

/// Attempts to find an existing entry chunk that can absorb modules shared between multiple entries.
///
/// This optimization reduces the number of chunks by merging shared modules into an appropriate
/// entry chunk when possible, rather than creating a separate common chunk.
///
/// Returns `Some(ChunkIdx)` if a suitable merge target is found, `None` otherwise.
pub fn try_insert_into_existing_chunk(
  chunk_idxs: &[ChunkIdx],
  entry_chunk_reference: &FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>>,
  chunk_graph: &ChunkGraph,
  module_table: &ModuleTable,
) -> Option<ChunkIdx> {
  let mut user_defined_entry = vec![];
  let mut dynamic_entry = vec![];
  for &idx in chunk_idxs {
    let Some(chunk) = chunk_graph.chunk_table.get(idx) else {
      continue;
    };
    match chunk.kind {
      ChunkKind::EntryPoint { meta, .. } => {
        if meta.contains(ChunkMeta::UserDefinedEntry) {
          user_defined_entry.push(idx);
        } else {
          dynamic_entry.push(idx);
        }
      }
      ChunkKind::Common => return None,
    }
  }
  let user_defined_entry_modules = collect_entry_modules(&user_defined_entry, chunk_graph)?;

  let merged_user_defined_chunk =
    find_merge_target(&user_defined_entry, &user_defined_entry_modules, module_table);
  if !user_defined_entry.is_empty() {
    let chunk_idx = merged_user_defined_chunk?;

    let ret = dynamic_entry.iter().all(|idx| {
      entry_chunk_reference
        .get(&chunk_idx)
        .map(|reached_dynamic_chunk| reached_dynamic_chunk.contains(idx))
        .unwrap_or(false)
    });
    return ret.then_some(chunk_idx);
  }

  let dynamic_chunk_entry_modules = collect_entry_modules(&dynamic_entry, chunk_graph)?;
  find_merge_target(&dynamic_entry, &dynamic_chunk_entry_modules, module_table)
}

/// Collects entry module indices from a list of chunk indices.
/// Returns `None` if any chunk is missing or has no entry module.
fn collect_entry_modules(
  chunk_indices: &[ChunkIdx],
  chunk_graph: &ChunkGraph,
) -> Option<Vec<ModuleIdx>> {
  let mut ret = Vec::with_capacity(chunk_indices.len());
  for chunk_idx in chunk_indices {
    let chunk = chunk_graph.chunk_table.get(*chunk_idx)?;
    ret.push(chunk.entry_module_idx()?);
  }
  Some(ret)
}

/// Finds a chunk that can serve as the merge target for all entries.
/// A chunk can be the merge target if its entry module is imported by or equal to all other entry modules.
fn find_merge_target(
  chunk_indices: &[ChunkIdx],
  entry_modules: &[ModuleIdx],
  module_table: &ModuleTable,
) -> Option<ChunkIdx> {
  chunk_indices.iter().zip(entry_modules.iter()).find_map(|(chunk_idx, entry_module_idx)| {
    let module = module_table[*entry_module_idx].as_normal().expect("Should be normal module");
    let can_merge = entry_modules.iter().all(|other_entry_module_idx| {
      *entry_module_idx == *other_entry_module_idx
        || module.importers_idx.contains(other_entry_module_idx)
    });
    can_merge.then_some(*chunk_idx)
  })
}
