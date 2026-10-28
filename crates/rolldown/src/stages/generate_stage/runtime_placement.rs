use std::collections::VecDeque;

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, EcmaViewMeta, OutputExports, OutputFormat,
  PostChunkOptimizationOperation, PreserveEntrySignatures, WrapKind,
};
use rustc_hash::FxHashSet;

use crate::chunk_graph::ChunkGraph;

use super::{
  GenerateStage,
  compute_cross_chunk_links::{ChunkImportOrder, CrossChunkLinkState},
  order_wrap_state::OrderWrapState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostProof {
  Local,
  Inert,
  Dependency,
}

pub(super) struct RuntimePlacement {
  #[cfg(debug_assertions)]
  pub(super) runtime_chunk: ChunkIdx,
  #[cfg(debug_assertions)]
  pub(super) host: ChunkIdx,
  #[cfg(debug_assertions)]
  pub(super) consumers: FxHashSet<ChunkIdx>,
}

impl GenerateStage<'_> {
  /// Select a host using the completed import graph. The order plan is retained across the move,
  /// so a helper edge cannot occupy a source import's position by accident.
  /// See internal-docs/code-splitting/implementation.md#runtime-module-placement.
  pub(super) fn fold_final_runtime_chunk(
    &mut self,
    graph: &mut ChunkGraph,
    links: &CrossChunkLinkState,
    order_state: &OrderWrapState,
    import_order: &mut ChunkImportOrder,
  ) -> Option<RuntimePlacement> {
    let runtime = self.link_output.runtime.id();
    if self.options.code_splitting.is_disabled() || self.link_output.entries.contains_key(&runtime)
    {
      return None;
    }
    let runtime_chunk = graph.module_to_chunk[runtime]?;
    if !graph.chunk_is_live(runtime_chunk)
      || graph.chunk_table[runtime_chunk].modules != [runtime]
      || links.has_dynamic_import_to(runtime_chunk)
      || self.link_output.metas[runtime].is_tla_or_contains_tla_dependency
    {
      return None;
    }
    if !import_order.physical[runtime_chunk.index()].is_empty() {
      return None;
    }
    let imports = &import_order.physical;
    let consumers = graph
      .chunk_table
      .iter_enumerated()
      .filter(|(idx, _)| {
        *idx != runtime_chunk
          && graph.chunk_is_live(*idx)
          && imports[idx.index()].contains(&runtime_chunk)
      })
      .map(|(idx, _)| idx)
      .collect::<FxHashSet<_>>();
    if consumers.is_empty() {
      return None;
    }

    let count = graph.chunk_table.len();
    let mut reverse: IndexVec<ChunkIdx, Vec<ChunkIdx>> = index_vec![vec![]; count];
    let mut remaining: IndexVec<ChunkIdx, usize> = index_vec![0; count];
    for (idx, _) in graph.chunk_table.iter_enumerated().filter(|(idx, _)| graph.chunk_is_live(*idx))
    {
      for &dependency in &imports[idx.index()] {
        if dependency != runtime_chunk {
          reverse[dependency].push(idx);
          remaining[idx] += 1;
        }
      }
    }
    // A host must not load another helper consumer before its own definitions exist.
    let forbidden = walk(consumers.iter().flat_map(|idx| reverse[*idx].iter().copied()), &reverse);
    let mut ready = graph
      .chunk_table
      .iter_enumerated()
      .filter(|(idx, _)| *idx != runtime_chunk && graph.chunk_is_live(*idx) && remaining[*idx] == 0)
      .map(|(idx, _)| idx)
      .collect::<VecDeque<_>>();
    let mut dag_order = vec![];
    while let Some(idx) = ready.pop_front() {
      dag_order.push(idx);
      for &parent in &reverse[idx] {
        remaining[parent] -= 1;
        if remaining[parent] == 0 {
          ready.push_back(parent);
        }
      }
    }
    let acyclic = dag_order.iter().copied().collect::<FxHashSet<_>>();
    let local_inert: IndexVec<ChunkIdx, bool> = graph
      .chunk_table
      .iter_enumerated()
      .map(|(idx, _)| {
        self.chunk_initialization_is_inert(graph, idx, order_state)
          && !links.has_external_imports(idx)
      })
      .collect();
    let rejected = walk(
      local_inert
        .iter_enumerated()
        .filter(|(idx, inert)| *idx != runtime_chunk && !**inert)
        .map(|(idx, _)| idx),
      &reverse,
    );
    let inert = graph
      .chunk_table
      .iter_enumerated()
      .filter(|(idx, _)| {
        *idx != runtime_chunk && graph.chunk_is_live(*idx) && !rejected.contains(idx)
      })
      .map(|(idx, _)| idx)
      .collect::<FxHashSet<_>>();

    let initial_roots = graph
      .chunk_table
      .iter()
      .filter_map(|chunk| {
        let ChunkKind::EntryPoint { module, meta, .. } = chunk.kind else {
          return None;
        };
        meta
          .intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk)
          .then(|| graph.entry_module_to_entry_chunk.get(&module).copied())
          .flatten()
      })
      .filter(|idx| graph.chunk_is_live(*idx))
      .collect::<FxHashSet<_>>();
    let single = (consumers.len() == 1).then(|| *consumers.iter().next().unwrap());
    let single_file = graph
      .chunk_table
      .iter_enumerated()
      .filter(|(idx, _)| *idx != runtime_chunk && graph.chunk_is_live(*idx))
      .count()
      == 1;
    // The first observable initialization on each acyclic import traversal must stay
    // after runtime initialization. An inert consumer is also a barrier: it can call a
    // helper while creating wrappers or namespaces.
    let mut first_effect: IndexVec<ChunkIdx, Option<ChunkIdx>> = index_vec![None; count];
    for &idx in &dag_order {
      first_effect[idx] = imports[idx.index()]
        .iter()
        .filter(|&&dep| dep != runtime_chunk)
        .find_map(|&dep| first_effect[dep])
        .or_else(|| (!local_inert[idx] || consumers.contains(&idx)).then_some(idx));
    }
    let mut candidates = vec![];
    for &idx in &inert {
      candidates.push((idx, HostProof::Inert));
    }
    if let Some(idx) = single {
      candidates.push((idx, HostProof::Local));
    }
    if consumers.iter().all(|idx| acyclic.contains(idx)) {
      let first_consumer = *consumers.iter().next().unwrap();
      if let Some(host) = first_effect[first_consumer]
        && consumers.iter().all(|&idx| first_effect[idx] == Some(host))
      {
        candidates.push((host, HostProof::Dependency));
      }
    }
    candidates.sort_by_cached_key(|&(idx, _proof)| {
      let chunk = &graph.chunk_table[idx];
      let non_leaf_local = single == Some(idx)
        && (links.has_external_imports(idx)
          || imports[idx.index()].iter().any(|&dep| dep != runtime_chunk));
      (
        non_leaf_local,
        chunk.bits != graph.chunk_table[runtime_chunk].bits,
        matches!(chunk.kind, ChunkKind::EntryPoint { .. }),
        chunk.exec_order,
        idx,
      )
    });
    let static_runtime_loaders = walk(consumers.iter().copied(), &reverse);
    // Inert definitions can still contain a large lazy subtree. Every entry that loads
    // helpers must already load the host through final static imports. Entry bits can
    // retain paths removed by namespace routing, so they cannot establish this boundary.
    let load_roots = initial_roots
      .iter()
      .copied()
      .chain(graph.chunk_table.iter_enumerated().flat_map(|(idx, _)| links.dynamic_importees(idx)))
      .filter(|idx| graph.chunk_is_live(*idx))
      .collect::<FxHashSet<_>>();
    let common_dependencies = common_dependencies(
      load_roots.iter().copied().filter(|idx| static_runtime_loaders.contains(idx)),
      imports,
      runtime_chunk,
    );
    let cold_roots = load_roots.into_iter().filter(|idx| !static_runtime_loaders.contains(idx));
    let cold_reachable = walk_forward(cold_roots, imports, runtime_chunk);
    let selected = candidates.into_iter().find(|&(idx, proof)| {
      if idx == runtime_chunk
        || !graph.chunk_is_live(idx)
        || forbidden.contains(&idx)
        || cold_reachable.contains(&idx)
      {
        return false;
      }
      if !self.runtime_signature_allows_host(graph, idx, &consumers) {
        return false;
      }
      if graph.chunk_table[idx]
        .modules
        .iter()
        .any(|&module| self.link_output.metas[module].is_tla_or_contains_tla_dependency)
        && single != Some(idx)
      {
        return false;
      }
      match proof {
        HostProof::Local => {
          single == Some(idx)
            && (single_file
              || !matches!(self.options.format, OutputFormat::Esm)
              || (acyclic.contains(&idx)
                && first_effect[idx] == Some(idx)
                && !links.has_external_imports(idx)))
        }
        HostProof::Inert => inert.contains(&idx) && common_dependencies.contains(&idx),
        HostProof::Dependency => {
          !matches!(self.options.format, OutputFormat::Esm) || !links.has_external_imports(idx)
        }
      }
    });
    let (host, proof) = selected?;
    self.apply_runtime_placement(graph, runtime_chunk, host, &consumers, import_order, proof);
    Some(RuntimePlacement {
      #[cfg(debug_assertions)]
      runtime_chunk,
      #[cfg(debug_assertions)]
      host,
      #[cfg(debug_assertions)]
      consumers,
    })
  }

  fn apply_runtime_placement(
    &mut self,
    graph: &mut ChunkGraph,
    runtime_chunk: ChunkIdx,
    host: ChunkIdx,
    consumers: &FxHashSet<ChunkIdx>,
    import_order: &mut ChunkImportOrder,
    proof: HostProof,
  ) {
    let runtime = self.link_output.runtime.id();
    for orders in [&mut import_order.physical, &mut import_order.logical] {
      for (raw_idx, order) in orders.iter_mut().enumerate() {
        let idx = ChunkIdx::from_usize(raw_idx);
        let position = order.iter().position(|&dep| dep == runtime_chunk).unwrap_or(0);
        let had_host = order.contains(&host);
        order.retain(|&dep| dep != runtime_chunk);
        if idx == host || !consumers.contains(&idx) {
          continue;
        }
        if proof == HostProof::Inert {
          order.retain(|&dep| dep != host);
          order.insert(position.min(order.len()), host);
        } else if !had_host {
          // A guaranteed static dependency is already evaluated by this point. This redundant
          // edge supplies the binding without changing the user's import traversal.
          order.push(host);
        }
      }
    }
    let helpers = std::mem::take(&mut graph.chunk_table[runtime_chunk].depended_runtime_helper);
    for &consumer in consumers {
      if let Some(items) =
        graph.chunk_table[consumer].imports_from_other_chunks.shift_remove(&runtime_chunk)
        && consumer != host
      {
        graph.chunk_table[consumer]
          .imports_from_other_chunks
          .entry(host)
          .or_default()
          .extend(items);
      }
    }
    graph.chunk_table[runtime_chunk].modules.clear();
    graph.add_module_to_chunk(runtime, host, helpers);
    graph
      .post_chunk_optimization_operations
      .insert(runtime_chunk, PostChunkOptimizationOperation::Removed);
    self.clear_module_symbol_chunk_indices(runtime);
    graph.sort_chunk_modules(self.link_output, self.options);
    graph.rebuild_sorted_chunk_idx_vec(true);
  }

  fn runtime_signature_allows_host(
    &self,
    graph: &ChunkGraph,
    host: ChunkIdx,
    consumers: &FxHashSet<ChunkIdx>,
  ) -> bool {
    if consumers.len() == 1 && consumers.contains(&host) {
      return true;
    }
    let chunk = &graph.chunk_table[host];
    let ChunkKind::EntryPoint { module, .. } = chunk.kind else {
      return !graph.common_chunk_preserve_export_names_modules.contains_key(&host);
    };
    if matches!(self.options.format, OutputFormat::Cjs)
      && chunk.user_defined_entry_module(&self.link_output.module_table).is_some()
    {
      // A default-only CJS entry assigns module.exports directly. Adding helper exports
      // would change that value to a namespace object, even with allow-extension.
      let exports = &self.link_output.metas[module].resolved_exports;
      if matches!(self.options.exports, OutputExports::Default | OutputExports::None)
        || (matches!(self.options.exports, OutputExports::Auto)
          && exports.len() == 1
          && exports.contains_key("default"))
        || self.link_output.module_table[module]
          .as_normal()
          .is_some_and(|module| module.exports_kind.is_commonjs())
      {
        return false;
      }
    }
    match chunk.preserve_entry_signature {
      Some(PreserveEntrySignatures::False | PreserveEntrySignatures::AllowExtension) => true,
      Some(PreserveEntrySignatures::Strict) => false,
      Some(PreserveEntrySignatures::ExportsOnly) | None => {
        self.link_output.metas[module].resolved_exports.is_empty()
      }
    }
  }

  fn chunk_initialization_is_inert(
    &self,
    graph: &ChunkGraph,
    idx: ChunkIdx,
    order: &OrderWrapState,
  ) -> bool {
    if self.options.is_dev_mode_enabled() {
      return false;
    }
    let chunk = &graph.chunk_table[idx];
    chunk.modules.iter().copied().all(|module_idx| {
      if module_idx == self.link_output.runtime.id() {
        return true;
      }
      let meta = &self.link_output.metas[module_idx];
      if matches!(chunk.kind, ChunkKind::Common)
        && (meta.wrap_kind() != WrapKind::None || order.has_order_wrapper(module_idx))
      {
        return true;
      }
      if meta.is_tla_or_contains_tla_dependency || !meta.shimmed_missing_exports.is_empty() {
        return false;
      }
      if meta.load_dependencies.iter().any(|&dependency| {
        self.link_output.metas[dependency].wrap_kind() != WrapKind::None
          || order.has_order_wrapper(dependency)
      }) {
        return false;
      }
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        return false;
      };
      module.meta.contains(EcmaViewMeta::IndependentInitialization)
        || self.ast_table[module_idx].as_ref().is_some_and(|ast| {
          crate::ast_scanner::independent_init::has_independent_initialization(
            ast.program(),
            &self.link_output.symbol_db[module_idx].as_ref().unwrap().ast_scopes,
            self.options.keep_names,
          )
        })
    })
  }
}

fn walk(
  starts: impl IntoIterator<Item = ChunkIdx>,
  edges: &IndexVec<ChunkIdx, Vec<ChunkIdx>>,
) -> FxHashSet<ChunkIdx> {
  let mut visited = FxHashSet::default();
  let mut stack = starts.into_iter().collect::<Vec<_>>();
  while let Some(idx) = stack.pop() {
    if visited.insert(idx) {
      stack.extend(edges[idx].iter().copied());
    }
  }
  visited
}

fn walk_forward(
  starts: impl IntoIterator<Item = ChunkIdx>,
  edges: &[Vec<ChunkIdx>],
  runtime: ChunkIdx,
) -> FxHashSet<ChunkIdx> {
  let mut visited = FxHashSet::default();
  let mut stack = starts.into_iter().collect::<Vec<_>>();
  while let Some(idx) = stack.pop() {
    if idx != runtime && visited.insert(idx) {
      stack.extend(edges[idx.index()].iter().copied());
    }
  }
  visited
}

fn common_dependencies(
  roots: impl IntoIterator<Item = ChunkIdx>,
  imports: &[Vec<ChunkIdx>],
  runtime: ChunkIdx,
) -> FxHashSet<ChunkIdx> {
  let mut roots = roots.into_iter();
  let Some(first) = roots.next() else {
    return FxHashSet::default();
  };
  let mut common = walk_forward([first], imports, runtime);
  for root in roots {
    if common.is_empty() {
      break;
    }
    let reachable = walk_forward([root], imports, runtime);
    common.retain(|idx| reachable.contains(idx));
  }
  common
}
