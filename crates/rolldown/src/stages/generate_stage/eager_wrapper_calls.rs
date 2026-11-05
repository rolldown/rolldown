use oxc_index::{IndexVec, index_vec};
use petgraph::graph::{DiGraph, NodeIndex};
use rolldown_common::{Chunk, ChunkIdx, ChunkKind, OutputFormat, WrapKind};
use rustc_hash::FxHashSet;

use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;

impl GenerateStage<'_> {
  /// This pass applies to `preserveModules` with ESM output.
  /// Rolldown normally writes an eager wrapper call in each wrapped module's chunk.
  /// This call is `export default require_x();` or `init_x();`.
  /// The call runs the module when ESM evaluates the chunk.
  ///
  /// In an import cycle, ESM evaluates the deepest member first.
  /// Its eager wrapper call can run another member's wrapper before that member defines the wrapper
  /// (#11348). A member keeps its eager wrapper call only in these cases:
  ///
  /// - It is the single way into the cycle.
  /// - It loads on its own.
  ///
  /// See internal-docs/linking/module-execution-order/implementation.md.
  pub(super) fn compute_chunks_without_eager_wrapper_call(&self, chunk_graph: &mut ChunkGraph) {
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
    // A way in is a member where ESM enters the cycle. A chunk outside the cycle imports it, or it
    // loads on its own.
    let mut is_way_in =
      chunk_table.iter().map(is_loaded_on_its_own).collect::<IndexVec<ChunkIdx, _>>();
    for edge in graph.raw_edges() {
      let importer = ChunkIdx::new(edge.source().index());
      let importee = ChunkIdx::new(edge.target().index());
      if cycle_of[importee].is_some() && cycle_of[importer] != cycle_of[importee] {
        is_way_in[importee] = true;
      }
    }

    let mut chunks_without_eager_wrapper_call = FxHashSet::default();
    for cycle in &cycles {
      let mut ways_in = cycle.iter().copied().filter(|member| is_way_in[*member]);
      let single_way_in = match (ways_in.next(), ways_in.next()) {
        (Some(way_in), None) => Some(way_in),
        _ => None,
      };
      for &member in cycle {
        let chunk = &chunk_table[member];
        // A chunk that loads on its own keeps its eager wrapper call, even if the cycle has more
        // ways in. Its importers read the `default` export, or they need the call to run the
        // module.
        if Some(member) == single_way_in || is_loaded_on_its_own(chunk) {
          continue;
        }
        let ChunkKind::EntryPoint { module, .. } = chunk.kind else { continue };
        if matches!(self.link_output.metas[module].wrap_kind(), WrapKind::Cjs | WrapKind::Esm) {
          chunks_without_eager_wrapper_call.insert(member);
        }
      }
    }
    chunk_graph.chunks_without_eager_wrapper_call = chunks_without_eager_wrapper_call;
  }
}

/// This function returns `true` for a chunk of any of these types:
///
/// - A user-defined entry.
/// - An emitted chunk.
/// - The target of a dynamic `import()`.
///
/// Such a chunk loads on its own.
fn is_loaded_on_its_own(chunk: &Chunk) -> bool {
  matches!(chunk.kind, ChunkKind::EntryPoint { meta, .. } if !meta.is_empty())
}
