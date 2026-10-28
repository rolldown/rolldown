use oxc_index::{IndexVec, index_vec};
use petgraph::graph::{DiGraph, NodeIndex};
use rolldown_common::{Chunk, ChunkIdx, ChunkKind, OutputFormat, WrapKind};
use rustc_hash::FxHashSet;

use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;

impl GenerateStage<'_> {
  /// With `preserveModules` and ESM output, a wrapped module runs at its chunk's tail call
  /// (`export default require_x();` or `init_x();`). ESM evaluates the deepest member of an import
  /// cycle first, so only the single way into a cycle keeps that call (#11348).
  /// See internal-docs/linking/module-execution-order/implementation.md.
  pub(super) fn compute_chunks_without_tail_wrapper_call(&self, chunk_graph: &mut ChunkGraph) {
    if !self.options.preserve_modules || !matches!(self.options.format, OutputFormat::Esm) {
      return;
    }
    let chunk_table = &chunk_graph.chunk_table;
    let mut graph = DiGraph::<(), ()>::with_capacity(chunk_table.len(), 0);
    for _ in chunk_table.iter() {
      graph.add_node(());
    }
    for (chunk_idx, chunk) in chunk_table.iter_enumerated() {
      for importee in chunk.imports_from_other_chunks.keys() {
        graph.add_edge(NodeIndex::new(chunk_idx.index()), NodeIndex::new(importee.index()), ());
      }
    }
    let cycles = petgraph::algo::kosaraju_scc(&graph)
      .into_iter()
      .filter(|component| component.len() > 1)
      .map(|component| {
        component.into_iter().map(|node| ChunkIdx::new(node.index())).collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    if cycles.is_empty() {
      return;
    }

    let mut cycle_of: IndexVec<ChunkIdx, Option<usize>> = index_vec![None; chunk_table.len()];
    for (cycle_idx, cycle) in cycles.iter().enumerate() {
      for &member in cycle {
        cycle_of[member] = Some(cycle_idx);
      }
    }
    // ESM enters a cycle at a member imported from outside it, or at one loaded on its own.
    let mut is_way_in =
      chunk_table.iter().map(is_loaded_on_its_own).collect::<IndexVec<ChunkIdx, _>>();
    for edge in graph.raw_edges() {
      let importer = ChunkIdx::new(edge.source().index());
      let importee = ChunkIdx::new(edge.target().index());
      if cycle_of[importee].is_some() && cycle_of[importer] != cycle_of[importee] {
        is_way_in[importee] = true;
      }
    }

    let mut chunks_without_tail_wrapper_call = FxHashSet::default();
    for cycle in &cycles {
      let mut ways_in = cycle.iter().copied().filter(|member| is_way_in[*member]);
      let single_way_in = match (ways_in.next(), ways_in.next()) {
        (Some(way_in), None) => Some(way_in),
        _ => None,
      };
      for &member in cycle {
        let chunk = &chunk_table[member];
        // A chunk loaded on its own keeps the call even with more ways in: its importers read the
        // `default` export or rely on the call to run the module.
        if Some(member) == single_way_in || is_loaded_on_its_own(chunk) {
          continue;
        }
        let ChunkKind::EntryPoint { module, .. } = chunk.kind else { continue };
        if matches!(self.link_output.metas[module].wrap_kind(), WrapKind::Cjs | WrapKind::Esm) {
          chunks_without_tail_wrapper_call.insert(member);
        }
      }
    }
    chunk_graph.chunks_without_tail_wrapper_call = chunks_without_tail_wrapper_call;
  }
}

/// A user-defined entry, an emitted chunk, or a dynamically imported chunk.
fn is_loaded_on_its_own(chunk: &Chunk) -> bool {
  matches!(chunk.kind, ChunkKind::EntryPoint { meta, .. } if !meta.is_empty())
}
