use oxc_index::IndexVec;
use rolldown_common::{ChunkIdx, CrossChunkImportItem};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{LOG_TARGET, place::compute_placement};
use crate::{chunk_graph::ChunkGraph, stages::generate_stage::GenerateStage};

/// Appends `importee` to `order`, or, when it is a record, its importees in their committed order
/// (records among them expanded the same way, each once).
fn expand_record_imports(
  importee: ChunkIdx,
  records: &FxIndexSet<ChunkIdx>,
  record_importees: &FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
  expanded: &mut FxHashSet<ChunkIdx>,
  order: &mut FxIndexSet<ChunkIdx>,
) {
  if !records.contains(&importee) {
    order.insert(importee);
    return;
  }
  if !expanded.insert(importee) {
    return;
  }
  for next in &record_importees[&importee] {
    expand_record_imports(*next, records, record_importees, expanded, order);
  }
}

impl GenerateStage<'_> {
  /// Projects the logical edges `commit_cross_chunk_links` just wrote onto the physical files.
  ///
  /// A record keeps its own imports (minus other records: records reach each other through
  /// bridges) so a carrier can print them. A file that reads records drops its imports of them,
  /// takes over the imports of every record it carries, and imports the runtime chunk first.
  pub(in crate::stages::generate_stage) fn apply_inline_common_chunks_links(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) {
    if !self.inline_state.has_records() {
      return;
    }
    let records = self.inline_state.record_indices().collect::<FxIndexSet<_>>();
    let static_importees: IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> = chunk_graph
      .chunk_table
      .iter()
      .map(|chunk| chunk.imports_from_other_chunks.keys().copied().collect())
      .collect();
    let placement = compute_placement(
      &static_importees,
      |chunk_idx| !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx),
      |chunk_idx| chunk_graph.chunk_table[chunk_idx].exec_order,
      &records,
    );
    // The `__share` and `__share_require` demands were registered from the selection-time
    // placement, so the final one must be the same table; it leaves no record without a carrier.
    assert!(
      placement == *self.inline_state.placement(),
      "inline common chunk placement changed between selection and final cross-chunk linking"
    );

    let runtime_chunk = self.inline_state.runtime_chunk().expect("records imply a runtime chunk");
    // A record's importees in their committed order, records included. A file expands every
    // record it reads at the position it imported the record, so the files behind the record,
    // externals included, still evaluate where they did before projection.
    let record_importees: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = records
      .iter()
      .map(|record| (*record, chunk_graph.chunk_table[*record].cross_chunk_imports.clone()))
      .collect();
    for record in &records {
      let chunk = &mut chunk_graph.chunk_table[*record];
      chunk.imports_from_other_chunks.retain(|importee, _| !records.contains(importee));
      chunk.cross_chunk_imports.retain(|importee| !records.contains(importee));
    }

    for file in placement.reading_files.iter().copied() {
      let carried = placement.carried.get(&file).map_or(&[][..], Vec::as_slice);
      let file_chunk = &chunk_graph.chunk_table[file];
      // Evaluation order: the runtime chunk first, then the file's committed importees in order,
      // each record expanded in place into its own committed importees. A record the file reads
      // without carrying it expands into bare imports.
      let mut order: FxIndexSet<ChunkIdx> = FxIndexSet::default();
      order.insert(runtime_chunk);
      let mut expanded: FxHashSet<ChunkIdx> = FxHashSet::default();
      for importee in &file_chunk.cross_chunk_imports {
        expand_record_imports(*importee, &records, &record_importees, &mut expanded, &mut order);
      }
      for record in carried {
        expand_record_imports(*record, &records, &record_importees, &mut expanded, &mut order);
      }
      // Import items: the file's own plus every carried record's, per importee. A carried record
      // cannot import the file: that would be a static cycle between a record and a file, which
      // selection keeps as files.
      let mut merged: FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>> = FxHashMap::default();
      for (importee, items) in &file_chunk.imports_from_other_chunks {
        if !records.contains(importee) {
          merged.entry(*importee).or_default().extend(items.iter().cloned());
        }
      }
      for record in carried {
        for (importee, items) in &chunk_graph.chunk_table[*record].imports_from_other_chunks {
          debug_assert!(*importee != file, "a carried record does not import its carrier");
          if !records.contains(importee) {
            merged.entry(*importee).or_default().extend(items.iter().cloned());
          }
        }
      }
      let imports_from_other_chunks = order
        .iter()
        .map(|importee| (*importee, merged.remove(importee).unwrap_or_default()))
        .collect::<Vec<_>>();
      debug_assert!(merged.is_empty(), "every importee with import items is in the import order");
      let cross_chunk_imports = order.into_iter().collect::<Vec<_>>();
      tracing::debug!(
        target: LOG_TARGET,
        file = file.raw(),
        carried = ?carried.iter().map(|idx| idx.raw()).collect::<Vec<_>>(),
        reads = ?placement.readers.get(&file).map(|read| read.iter().map(|idx| idx.raw()).collect::<Vec<_>>()),
        "file takes over the imports of the records it carries"
      );

      let chunk = &mut chunk_graph.chunk_table[file];
      chunk.imports_from_other_chunks = imports_from_other_chunks.into_iter().collect();
      chunk.cross_chunk_imports = cross_chunk_imports;
    }

    // Every edge to a record was replaced above; a file that still had one would import a file
    // that is never written.
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
        || records.contains(&chunk_idx)
      {
        continue;
      }
      assert!(
        !chunk.cross_chunk_imports.iter().any(|importee| records.contains(importee))
          && !chunk.imports_from_other_chunks.keys().any(|importee| records.contains(importee)),
        "file {chunk_idx:?} still imports an inline common chunk record after projection"
      );
    }
  }
}
