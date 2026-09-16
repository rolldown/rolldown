use oxc_index::IndexVec;
use rolldown_common::ChunkIdx;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashSet;

use super::{LOG_TARGET, place::compute_placement};
use crate::{chunk_graph::ChunkGraph, stages::generate_stage::GenerateStage};

impl GenerateStage<'_> {
  /// Projects the logical edges `compute_cross_chunk_links` just wrote onto the physical files.
  ///
  /// A record keeps its own imports (minus other records: records reach each other through
  /// bridges) so a carrier can print them. A carrier drops its imports of records, takes over the
  /// imports of every record it carries, and imports the runtime chunk first.
  pub(in crate::stages::generate_stage) fn apply_inline_common_chunks_links(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) {
    if !self.inline_state.has_records() {
      return;
    }
    let records = self.inline_state.record_indices().collect::<FxIndexSet<_>>();
    let static_importees: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> = chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, chunk)| {
        if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx) {
          return FxHashSet::default();
        }
        chunk.imports_from_other_chunks.keys().copied().filter(|idx| *idx != chunk_idx).collect()
      })
      .collect();
    let placement = compute_placement(chunk_graph, &static_importees, &records);
    for record in &records {
      assert!(
        placement.is_carried(*record),
        "inline common chunk {record:?} is read by no file after final cross-chunk linking"
      );
    }
    debug_assert!(
      records.iter().all(|record| {
        self.inline_state.readers_of(*record)
          == placement.readers.get(record).map_or(&[][..], Vec::as_slice)
      }),
      "record -> record read edges changed between selection and final cross-chunk linking"
    );

    let runtime_chunk = self.inline_state.runtime_chunk().expect("records imply a runtime chunk");
    for record in &records {
      let chunk = &mut chunk_graph.chunk_table[*record];
      chunk.imports_from_other_chunks.retain(|importee, _| !records.contains(importee));
      chunk.cross_chunk_imports.retain(|importee| !records.contains(importee));
    }

    let mut carriers = placement.carried.keys().copied().collect::<Vec<_>>();
    carriers.sort_unstable();
    for carrier in carriers {
      let carried = &placement.carried[&carrier];
      let mut cross_chunk_imports =
        chunk_graph.chunk_table[carrier]
          .cross_chunk_imports
          .iter()
          .copied()
          .filter(|importee| !records.contains(importee))
          .chain(carried.iter().flat_map(|record| {
            chunk_graph.chunk_table[*record].cross_chunk_imports.iter().copied()
          }))
          .chain(std::iter::once(runtime_chunk))
          .filter(|importee| *importee != carrier)
          .collect::<FxIndexSet<_>>()
          .into_iter()
          .collect::<Vec<_>>();
      cross_chunk_imports.sort_unstable_by_key(|importee| {
        (*importee != runtime_chunk, chunk_graph.chunk_table[*importee].exec_order)
      });
      tracing::debug!(
        target: LOG_TARGET,
        carrier = carrier.raw(),
        carried = ?carried.iter().map(|idx| idx.raw()).collect::<Vec<_>>(),
        reads = ?placement.readers.get(&carrier).map(|read| read.iter().map(|idx| idx.raw()).collect::<Vec<_>>()),
        "carrier takes over record imports"
      );

      let chunk = &mut chunk_graph.chunk_table[carrier];
      chunk.imports_from_other_chunks.retain(|importee, _| !records.contains(importee));
      if !chunk.imports_from_other_chunks.contains_key(&runtime_chunk) {
        chunk.imports_from_other_chunks.insert(runtime_chunk, vec![]);
      }
      let runtime_position = chunk
        .imports_from_other_chunks
        .get_index_of(&runtime_chunk)
        .expect("runtime chunk was inserted above");
      chunk.imports_from_other_chunks.move_index(runtime_position, 0);
      chunk.cross_chunk_imports = cross_chunk_imports;
    }

    self.inline_state.set_placement(placement);
  }
}
