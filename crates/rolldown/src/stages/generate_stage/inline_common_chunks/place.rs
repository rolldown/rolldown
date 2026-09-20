use oxc_index::IndexVec;
use rolldown_common::ChunkIdx;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

/// Who reads and who carries each record, derived from a chunk -> importee edge table.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct InlinePlacement {
  /// File or record -> the records it reads directly (record execution order).
  pub readers: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  /// File -> every record reachable from the records it reads along record -> record read
  /// edges, dependencies first. A record therefore has a factory in every file whose code can
  /// require it, directly or through another record's factory.
  pub carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
}

impl InlinePlacement {
  pub fn is_carried(&self, record: ChunkIdx) -> bool {
    self.carried.values().any(|carried| carried.contains(&record))
  }
}

pub(super) fn compute_placement(
  chunk_graph: &ChunkGraph,
  static_importees: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  records: &FxIndexSet<ChunkIdx>,
) -> InlinePlacement {
  let mut readers: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
  for (chunk_idx, importees) in static_importees.iter_enumerated() {
    if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx) {
      continue;
    }
    let mut read = importees
      .iter()
      .copied()
      .filter(|importee| *importee != chunk_idx && records.contains(importee))
      .collect::<Vec<_>>();
    if read.is_empty() {
      continue;
    }
    read.sort_unstable_by_key(|idx| (chunk_graph.chunk_table[*idx].exec_order, idx.raw()));
    readers.insert(chunk_idx, read);
  }

  let mut carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
  for (chunk_idx, read) in &readers {
    if records.contains(chunk_idx) {
      continue;
    }
    let mut visited = FxHashSet::default();
    let mut order = Vec::new();
    for record in read {
      visit(*record, &readers, &mut visited, &mut order);
    }
    carried.insert(*chunk_idx, order);
  }

  InlinePlacement { readers, carried }
}

fn visit(
  record: ChunkIdx,
  readers: &FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  visited: &mut FxHashSet<ChunkIdx>,
  order: &mut Vec<ChunkIdx>,
) {
  if !visited.insert(record) {
    return;
  }
  for dependency in readers.get(&record).into_iter().flatten() {
    visit(*dependency, readers, visited, order);
  }
  order.push(record);
}
