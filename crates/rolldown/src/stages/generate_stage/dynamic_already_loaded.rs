use std::collections::VecDeque;

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx,
  NormalModule, PreserveEntrySignatures, RuntimeHelper, StmtInfo, StmtInfoIdx, StmtInfoMeta,
  SymbolOrMemberExprRef, SymbolRef, UsedSymbolRefsBuilder, UsedSymbolRefsView, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage, side_effects::DeterminedSideEffects,
};
use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;
use crate::esm_init_obligations::collect_entry_reexported_wrapper_inits;

use super::{
  GenerateStage, chunk_optimizer::side_effect_exec_order_range, code_splitting::IndexSplittingInfo,
  simulated_facade_inclusion::include_simulated_facade_namespace,
};

struct ChunkAtom {
  modules: Vec<ModuleIdx>,
  dependent_entries: BitSet,
}

/// Atom-level edge graphs for the pass's two consumers. `reachability` under-approximates the
/// emitted chunk imports (safe to miss an edge, never to invent one); `cycle` over-approximates
/// them (safe to invent an edge, never to miss one). See `compute_atom_dependencies`.
struct AtomDependencyGraphs {
  reachability: Vec<Vec<usize>>,
  cycle: Vec<Vec<usize>>,
}

/// Chunk edges with the number of atom (or facade) edges behind each, so that moving one atom
/// adds and removes exactly the edges it contributes.
type CountedChunkEdges = Vec<FxHashMap<usize, u32>>;

/// The chunk graph the atoms' current dependent-entry bits produce: atoms grouped by bits, with
/// the facade edges the atom graph cannot express. Built once per pass and kept in step with the
/// folds as they are tried (see `move_atom`), so the cycle and the execution-order checks read
/// one graph and no candidate pays for rebuilding it: a rebuild hashes every atom's `BitSet`,
/// which made the pass quadratic in the atoms with a factor of the entry count.
struct ReducedChunkGraph<'a> {
  atom_dependencies: &'a AtomDependencyGraphs,
  host_atom_by_entry_bit: &'a [Option<usize>],
  atom_side_effect_exec_orders: &'a [Option<(u32, u32)>],
  atom_to_chunk: Vec<usize>,
  chunk_idx_by_bits: FxHashMap<BitSet, usize>,
  /// The atoms of every chunk. A chunk every atom has left stays as a node without edges.
  chunk_atoms: Vec<Vec<usize>>,
  /// The entry bit of a chunk whose bits are that one bit: the entry's own chunk, a facade when
  /// its module is hosted elsewhere.
  chunk_entry_bit: Vec<Option<u32>>,
  /// The chunk whose bits are exactly each entry's bit, once an atom has had those bits.
  facade_chunk_by_entry_bit: Vec<Option<usize>>,
  /// Chunk edges from the over-approximated `cycle` atom graph plus the facade edges.
  chunk_dependencies: CountedChunkEdges,
  /// Chunk edges from the under-approximated `reachability` atom graph: imports the emitted
  /// chunks are certain to have.
  certain_chunk_dependencies: CountedChunkEdges,
  /// The latest execution order among each chunk's side-effectful modules.
  chunk_latest_side_effect: Vec<Option<u32>>,
  /// The atoms with an edge to each atom, per graph, so a move re-points the edges into it.
  cycle_dependents: Vec<Vec<usize>>,
  reachability_dependents: Vec<Vec<usize>>,
  /// The entry bits each atom hosts (it holds the entry module), whose facade edges point at it.
  hosted_entry_bits: Vec<Vec<u32>>,
}

fn add_chunk_edge(edges: &mut CountedChunkEdges, from_chunk_idx: usize, to_chunk_idx: usize) {
  if from_chunk_idx != to_chunk_idx {
    *edges[from_chunk_idx].entry(to_chunk_idx).or_insert(0) += 1;
  }
}

fn remove_chunk_edge(edges: &mut CountedChunkEdges, from_chunk_idx: usize, to_chunk_idx: usize) {
  if from_chunk_idx == to_chunk_idx {
    return;
  }
  let count = edges[from_chunk_idx]
    .get_mut(&to_chunk_idx)
    .expect("every chunk edge removed was added for the same atom or facade");
  *count -= 1;
  if *count == 0 {
    edges[from_chunk_idx].remove(&to_chunk_idx);
  }
}

fn dependents_of(dependencies: &[Vec<usize>]) -> Vec<Vec<usize>> {
  let mut dependents = vec![vec![]; dependencies.len()];
  for (atom_idx, atom_dependencies) in dependencies.iter().enumerate() {
    for &dep_atom_idx in atom_dependencies {
      dependents[dep_atom_idx].push(atom_idx);
    }
  }
  dependents
}

impl<'a> ReducedChunkGraph<'a> {
  fn new(
    atoms: &[ChunkAtom],
    atom_dependencies: &'a AtomDependencyGraphs,
    host_atom_by_entry_bit: &'a [Option<usize>],
    atom_side_effect_exec_orders: &'a [Option<(u32, u32)>],
  ) -> Self {
    let mut hosted_entry_bits = vec![vec![]; atoms.len()];
    for (entry_bit, host_atom_idx) in host_atom_by_entry_bit.iter().enumerate() {
      if let Some(host_atom_idx) = *host_atom_idx {
        hosted_entry_bits[host_atom_idx]
          .push(entry_bit.try_into().expect("Too many entries, u32 overflowed."));
      }
    }
    let mut graph = Self {
      atom_dependencies,
      host_atom_by_entry_bit,
      atom_side_effect_exec_orders,
      atom_to_chunk: Vec::with_capacity(atoms.len()),
      chunk_idx_by_bits: FxHashMap::default(),
      chunk_atoms: vec![],
      chunk_entry_bit: vec![],
      facade_chunk_by_entry_bit: vec![None; host_atom_by_entry_bit.len()],
      chunk_dependencies: vec![],
      certain_chunk_dependencies: vec![],
      chunk_latest_side_effect: vec![],
      cycle_dependents: dependents_of(&atom_dependencies.cycle),
      reachability_dependents: dependents_of(&atom_dependencies.reachability),
      hosted_entry_bits,
    };
    for (atom_idx, atom) in atoms.iter().enumerate() {
      let chunk_idx = graph.chunk_for(&atom.dependent_entries);
      graph.atom_to_chunk.push(chunk_idx);
      graph.chunk_atoms[chunk_idx].push(atom_idx);
      graph.widen_latest_side_effect(chunk_idx, atom_idx);
    }
    for (atom_idx, dependencies) in atom_dependencies.cycle.iter().enumerate() {
      for &dep_atom_idx in dependencies {
        add_chunk_edge(
          &mut graph.chunk_dependencies,
          graph.atom_to_chunk[atom_idx],
          graph.atom_to_chunk[dep_atom_idx],
        );
      }
    }
    for (atom_idx, dependencies) in atom_dependencies.reachability.iter().enumerate() {
      for &dep_atom_idx in dependencies {
        add_chunk_edge(
          &mut graph.certain_chunk_dependencies,
          graph.atom_to_chunk[atom_idx],
          graph.atom_to_chunk[dep_atom_idx],
        );
      }
    }
    // The facade edges the atom graph cannot express: an entry whose module lands in a shared chunk
    // keeps its own entry chunk as a facade, and `compute_chunk_imports` makes that facade import
    // the host chunk — to run the module and to re-export its interface — but no module in the
    // facade imports the entry module. Without them, folding an atom into a facade whose host
    // statically depends on that atom yields a chunk cycle that evaluates in the wrong order
    // (rolldown#10734). `cycle` over-approximates, so a redundant facade edge only costs a missed
    // merge.
    for chunk_idx in 0..graph.chunk_atoms.len() {
      if let Some(entry_bit) = graph.chunk_entry_bit[chunk_idx] {
        graph.add_facade_edge(entry_bit);
      }
    }
    graph
  }

  /// The chunk of `bits`, created as an empty node when no atom has had these bits yet.
  fn chunk_for(&mut self, bits: &BitSet) -> usize {
    if let Some(&chunk_idx) = self.chunk_idx_by_bits.get(bits) {
      return chunk_idx;
    }
    let chunk_idx = self.chunk_atoms.len();
    self.chunk_idx_by_bits.insert(bits.clone(), chunk_idx);
    self.chunk_atoms.push(vec![]);
    let entry_bit = if bits.bit_count() == 1 { bits.index_of_one().next() } else { None };
    if let Some(entry_bit) = entry_bit {
      self.facade_chunk_by_entry_bit[entry_bit as usize] = Some(chunk_idx);
    }
    self.chunk_entry_bit.push(entry_bit);
    self.chunk_dependencies.push(FxHashMap::default());
    self.certain_chunk_dependencies.push(FxHashMap::default());
    self.chunk_latest_side_effect.push(None);
    chunk_idx
  }

  /// The entry's own chunk when it holds an atom. A module-less facade declares no symbol, so
  /// nothing can depend back on it and the omitted node cannot sit on a cycle.
  fn facade_chunk(&self, entry_bit: u32) -> Option<usize> {
    self.facade_chunk_by_entry_bit[entry_bit as usize]
      .filter(|&chunk_idx| !self.chunk_atoms[chunk_idx].is_empty())
  }

  /// The chunk hosting the entry's module. A `None` host means the entry module is external or
  /// excluded. Skipping is not a safe approximation in general — `cycle` may never miss an edge —
  /// but it drops no real edge: `split_chunks` assigns such a module to no chunk, so the facade has
  /// no host to import. An included normal entry module always has an atom —
  /// `determine_reachable_modules_for_entry` sets the entry's own bit on its module, and
  /// `split_chunks` asserts that an included module's bits are non-empty.
  fn host_chunk(&self, entry_bit: u32) -> Option<usize> {
    self.host_atom_by_entry_bit[entry_bit as usize]
      .map(|host_atom_idx| self.atom_to_chunk[host_atom_idx])
  }

  fn add_facade_edge(&mut self, entry_bit: u32) {
    if let (Some(facade_chunk_idx), Some(host_chunk_idx)) =
      (self.facade_chunk(entry_bit), self.host_chunk(entry_bit))
    {
      add_chunk_edge(&mut self.chunk_dependencies, facade_chunk_idx, host_chunk_idx);
    }
  }

  fn remove_facade_edge(&mut self, entry_bit: u32) {
    if let (Some(facade_chunk_idx), Some(host_chunk_idx)) =
      (self.facade_chunk(entry_bit), self.host_chunk(entry_bit))
    {
      remove_chunk_edge(&mut self.chunk_dependencies, facade_chunk_idx, host_chunk_idx);
    }
  }

  fn widen_latest_side_effect(&mut self, chunk_idx: usize, atom_idx: usize) {
    if let Some((_, latest)) = self.atom_side_effect_exec_orders[atom_idx] {
      let known = &mut self.chunk_latest_side_effect[chunk_idx];
      *known = Some(known.map_or(latest, |known: u32| known.max(latest)));
    }
  }

  /// Moves `atom_idx` to the chunk of `bits`: a fold being tried, or taken back when vetoed. The
  /// atom's own edges, the edges into it and the facade edges that point at or leave its chunks
  /// are re-pointed; the chunks' latest side effects follow.
  fn move_atom(&mut self, atom_idx: usize, bits: &BitSet) {
    let from_chunk_idx = self.atom_to_chunk[atom_idx];
    let to_chunk_idx = self.chunk_for(bits);
    if from_chunk_idx == to_chunk_idx {
      return;
    }
    // The facade edges the move retires, read while the atom is still in its chunk: those pointing
    // at the chunk as the host of an entry the atom holds, and the one leaving the chunk when the
    // atom is the last member of a facade.
    for entry_bit in self.hosted_entry_bits[atom_idx].clone() {
      self.remove_facade_edge(entry_bit);
    }
    if self.chunk_atoms[from_chunk_idx].len() == 1
      && let Some(entry_bit) = self.chunk_entry_bit[from_chunk_idx]
    {
      self.remove_facade_edge(entry_bit);
    }
    let to_chunk_was_empty = self.chunk_atoms[to_chunk_idx].is_empty();
    let position = self.chunk_atoms[from_chunk_idx]
      .iter()
      .position(|&member| member == atom_idx)
      .expect("an atom is a member of its chunk");
    self.chunk_atoms[from_chunk_idx].swap_remove(position);
    self.chunk_atoms[to_chunk_idx].push(atom_idx);
    self.atom_to_chunk[atom_idx] = to_chunk_idx;

    self.retarget_atom_edges(atom_idx, from_chunk_idx, to_chunk_idx, false);
    self.retarget_atom_edges(atom_idx, from_chunk_idx, to_chunk_idx, true);

    self.widen_latest_side_effect(to_chunk_idx, atom_idx);
    self.chunk_latest_side_effect[from_chunk_idx] = None;
    for member in self.chunk_atoms[from_chunk_idx].clone() {
      self.widen_latest_side_effect(from_chunk_idx, member);
    }

    // The facade edges the move creates: those pointing at the new chunk as the host of an entry
    // the atom holds, and the one leaving a facade the atom is the first member of.
    for entry_bit in self.hosted_entry_bits[atom_idx].clone() {
      self.add_facade_edge(entry_bit);
    }
    if to_chunk_was_empty && let Some(entry_bit) = self.chunk_entry_bit[to_chunk_idx] {
      self.add_facade_edge(entry_bit);
    }
  }

  /// Re-points the edges `atom_idx` contributes to one chunk graph after its move. An edge between
  /// two atoms of one chunk is no chunk edge, so the count only changes where the move separates
  /// or joins the two ends.
  fn retarget_atom_edges(
    &mut self,
    atom_idx: usize,
    from_chunk_idx: usize,
    to_chunk_idx: usize,
    certain: bool,
  ) {
    let (dependencies, dependents) = if certain {
      (&self.atom_dependencies.reachability, &self.reachability_dependents)
    } else {
      (&self.atom_dependencies.cycle, &self.cycle_dependents)
    };
    let edges =
      if certain { &mut self.certain_chunk_dependencies } else { &mut self.chunk_dependencies };
    for &dep_atom_idx in &dependencies[atom_idx] {
      let dep_chunk_idx = self.atom_to_chunk[dep_atom_idx];
      remove_chunk_edge(edges, from_chunk_idx, dep_chunk_idx);
      add_chunk_edge(edges, to_chunk_idx, dep_chunk_idx);
    }
    for &dependent_atom_idx in &dependents[atom_idx] {
      let dependent_chunk_idx = self.atom_to_chunk[dependent_atom_idx];
      remove_chunk_edge(edges, dependent_chunk_idx, from_chunk_idx);
      add_chunk_edge(edges, dependent_chunk_idx, to_chunk_idx);
    }
  }

  /// Whether `start_chunk_idx` sits on a cycle. Moving one atom only adds edges at its new chunk,
  /// so every cycle a fold can create runs through that chunk. A cycle elsewhere in the
  /// over-approximated graph predates the fold and must not veto it.
  fn chunk_reaches_itself(&self, start_chunk_idx: usize) -> bool {
    let mut visited = vec![false; self.chunk_dependencies.len()];
    let mut stack = self.chunk_dependencies[start_chunk_idx].keys().copied().collect::<Vec<_>>();
    while let Some(chunk_idx) = stack.pop() {
      if chunk_idx == start_chunk_idx {
        return true;
      }
      if !std::mem::replace(&mut visited[chunk_idx], true) {
        stack.extend(self.chunk_dependencies[chunk_idx].keys().copied());
      }
    }
    false
  }

  /// Whether folding `folded_atom_idx` runs one of its side effects after a side effect that used
  /// to run later. The atom's modules now run in the body of their new chunk, after every chunk
  /// that chunk imports; a side-effectful module in that import closure with a later execution
  /// order than the atom's earliest side effect ran after the atom before the fold. Chunks the
  /// atom is certain to import itself are exempt: they ran before the atom's chunk already. The
  /// exemption reads the under-approximated edges, so an invented import never excuses a chunk,
  /// while the closure reads the over-approximated ones, so no real import is missed.
  fn fold_runs_side_effects_late(&self, folded_atom_idx: usize) -> bool {
    let Some((earliest_side_effect, _)) = self.atom_side_effect_exec_orders[folded_atom_idx] else {
      return false;
    };
    let folded_chunk_idx = self.atom_to_chunk[folded_atom_idx];
    let already_before = Self::reachable_from(
      &self.certain_chunk_dependencies,
      self.atom_dependencies.reachability[folded_atom_idx]
        .iter()
        .map(|&dep_atom_idx| self.atom_to_chunk[dep_atom_idx]),
      folded_chunk_idx,
    );
    let mut visited = vec![false; self.chunk_dependencies.len()];
    let mut stack = self.chunk_dependencies[folded_chunk_idx].keys().copied().collect::<Vec<_>>();
    while let Some(chunk_idx) = stack.pop() {
      if chunk_idx == folded_chunk_idx || std::mem::replace(&mut visited[chunk_idx], true) {
        continue;
      }
      if !already_before[chunk_idx]
        && self.chunk_latest_side_effect[chunk_idx]
          .is_some_and(|latest_side_effect| latest_side_effect > earliest_side_effect)
      {
        return true;
      }
      stack.extend(self.chunk_dependencies[chunk_idx].keys().copied());
    }
    false
  }

  /// The chunks reachable from `roots` through `dependencies`, `skip` excluded.
  fn reachable_from(
    dependencies: &CountedChunkEdges,
    roots: impl Iterator<Item = usize>,
    skip: usize,
  ) -> Vec<bool> {
    let mut reached = vec![false; dependencies.len()];
    let mut stack = roots.filter(|&chunk_idx| chunk_idx != skip).collect::<Vec<_>>();
    while let Some(chunk_idx) = stack.pop() {
      if chunk_idx == skip || std::mem::replace(&mut reached[chunk_idx], true) {
        continue;
      }
      stack.extend(dependencies[chunk_idx].keys().copied());
    }
    reached
  }
}

struct DynamicEntryAnalysis {
  dynamic_entry_indices: Vec<usize>,
  dynamic_entry_modules_by_entry: Vec<Option<ModuleIdx>>,
  dynamic_imports_by_entry: Vec<BitSet>,
  dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
}

enum ReducedEntriesAction {
  Avoid,
  Apply,
  // The atom can be added to the dynamic entry chunk, but it needs care to preserve
  // its namespace due to it being dynamically imported.
  ApplyWithNamespaceExtraction { entry_chunk_idx: ChunkIdx, entry_module_idx: ModuleIdx },
}

impl ReducedEntriesAction {
  fn apply_if(apply: bool) -> Self {
    if apply { ReducedEntriesAction::Apply } else { ReducedEntriesAction::Avoid }
  }
}

impl GenerateStage<'_> {
  pub(super) fn optimize_dynamic_entry_bits(
    &mut self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    entries_len: u32,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
    own_side_effect_exec_orders: &IndexVec<ModuleIdx, Option<u32>>,
  ) {
    let mut namespace_extractions = FxHashSet::default();
    let DynamicEntryAnalysis {
      dynamic_entry_indices,
      dynamic_entry_modules_by_entry,
      dynamic_imports_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
    } = self.analyze_dynamic_entries(index_splitting_info, entries_len);
    if dynamic_entry_indices.is_empty() {
      return;
    }

    let mut atoms = self.group_modules_by_dependent_entries(index_splitting_info);
    if atoms.is_empty() {
      return;
    }
    let module_to_atom_idx = self.compute_module_to_atom_idx(&atoms);
    let flattened_entry_modules = self.flattened_entry_modules();
    let atom_dependencies = self.compute_atom_dependencies(
      &atoms,
      &module_to_atom_idx,
      &flattened_entry_modules,
      used_symbol_refs_builder,
    );
    // Indexed by entry bit, so `ReducedChunkGraph` can pair each facade chunk with its entry
    // module's host atom by index instead of hashing a synthesized `BitSet`.
    let host_atom_by_entry_bit: Vec<Option<usize>> =
      flattened_entry_modules.iter().map(|&module_idx| module_to_atom_idx[module_idx]).collect();

    let static_dependency_atoms_by_entry = self.compute_static_dependency_atoms_by_entry(
      entries_len as usize,
      &atoms,
      &atom_dependencies.reachability,
      &module_to_atom_idx,
    );
    let already_loaded_atoms_by_entry = Self::compute_already_loaded_atoms_by_entry(
      &static_dependency_atoms_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
      &dynamic_imports_by_entry,
      &dynamic_entry_indices,
      atoms.len(),
    );

    let strict_execution_order = self.options.is_strict_execution_order_enabled();
    let atom_side_effect_exec_orders: Vec<Option<(u32, u32)>> = atoms
      .iter()
      .map(|atom| {
        side_effect_exec_order_range(atom.modules.iter().copied(), own_side_effect_exec_orders)
      })
      .collect();

    let mut reduced_graph = ReducedChunkGraph::new(
      &atoms,
      &atom_dependencies,
      &host_atom_by_entry_bit,
      &atom_side_effect_exec_orders,
    );
    let mut changed = false;
    let mut pending_atoms: Vec<usize> = (0..atoms.len()).collect();
    loop {
      let mut folded_this_round = false;
      let mut deferred_atoms = vec![];
      for atom_idx in pending_atoms {
        let original_dependent_entries = atoms[atom_idx].dependent_entries.clone();
        let dependent_entries =
          atoms[atom_idx].dependent_entries.index_of_one().collect::<Vec<_>>();
        let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
        for entry_idx in dependent_entries {
          if already_loaded_atoms_by_entry[entry_idx as usize].has_bit(atom_bit) {
            atoms[atom_idx].dependent_entries.clear_bit(entry_idx);
          }
        }
        let action = if atoms[atom_idx].dependent_entries == original_dependent_entries {
          ReducedEntriesAction::Avoid
        } else {
          self.can_use_reduced_dependent_entries(
            &atoms[atom_idx],
            &original_dependent_entries,
            &atoms[atom_idx].dependent_entries,
            chunk_graph,
            &dynamic_entry_modules_by_entry,
          )
        };
        if matches!(action, ReducedEntriesAction::Avoid) {
          atoms[atom_idx].dependent_entries = original_dependent_entries;
          continue;
        }
        // The fold is tried on the graph and taken back below when a check vetoes it.
        reduced_graph.move_atom(atom_idx, &atoms[atom_idx].dependent_entries);
        let acyclic = !reduced_graph.chunk_reaches_itself(reduced_graph.atom_to_chunk[atom_idx]);
        let runs_side_effects_late =
          acyclic && !strict_execution_order && reduced_graph.fold_runs_side_effects_late(atom_idx);
        if acyclic && !runs_side_effects_late {
          changed = true;
          folded_this_round = true;
          if let ReducedEntriesAction::ApplyWithNamespaceExtraction {
            entry_chunk_idx,
            entry_module_idx,
          } = action
          {
            namespace_extractions.insert((entry_chunk_idx, entry_module_idx));
          }
        } else {
          reduced_graph.move_atom(atom_idx, &original_dependent_entries);
          atoms[atom_idx].dependent_entries = original_dependent_entries;
          // The chunk whose side effect would overtake this atom may itself fold into the same
          // body later in the round, which removes the hazard; such an atom gets another look.
          if runs_side_effects_late {
            deferred_atoms.push(atom_idx);
          }
        }
      }
      if deferred_atoms.is_empty() || !folded_this_round {
        break;
      }
      pending_atoms = deferred_atoms;
    }

    if !changed {
      debug_assert!(namespace_extractions.is_empty());
      return;
    }

    for atom in atoms {
      let share_count = atom.dependent_entries.bit_count();
      for module_idx in atom.modules {
        index_splitting_info[module_idx].bits = atom.dependent_entries.clone();
        index_splitting_info[module_idx].share_count = share_count;
      }
    }

    self.apply_dynamic_entry_namespace_extractions(
      chunk_graph,
      &namespace_extractions,
      used_symbol_refs_builder,
    );
  }

  fn can_use_reduced_dependent_entries(
    &self,
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    chunk_graph: &ChunkGraph,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> ReducedEntriesAction {
    let bit_count = dependent_entries.bit_count();
    if bit_count != 1 {
      return ReducedEntriesAction::apply_if(bit_count > 1);
    }

    let Some(entry_bit) = dependent_entries.index_of_one().next() else {
      return ReducedEntriesAction::Avoid;
    };
    let entry_chunk_idx = ChunkIdx::from_raw(entry_bit);
    let Some(chunk) = chunk_graph.chunk_table.get(entry_chunk_idx) else {
      return ReducedEntriesAction::Avoid;
    };

    // See internal-docs/inline-common-chunks/design.md#keeping-shared-code-available-for-inlining.
    if self.prefers_inline_common_chunk(&atom.modules) {
      return ReducedEntriesAction::Avoid;
    }

    let can_merge_without_changing_entry_signature =
      self.can_merge_without_changing_entry_signature(chunk, &atom.modules);
    let is_runtime_only_atom = self.is_runtime_only_atom(atom);
    let removed_entries_are_dynamic_entry_modules = Self::removed_entries_are_dynamic_entry_modules(
      atom,
      original_dependent_entries,
      dependent_entries,
      dynamic_entry_modules_by_entry,
    );

    if can_merge_without_changing_entry_signature
      || is_runtime_only_atom
      || removed_entries_are_dynamic_entry_modules
    {
      return ReducedEntriesAction::Apply;
    }

    // If a chunk is a dynamic entry point, we may still inline into it by taking care of the namespace.
    if let ChunkKind::EntryPoint { meta, module: entry_module_idx, .. } = chunk.kind
      && meta == ChunkMeta::DynamicImported
    {
      if self.dynamic_entry_partial_usage_allows_plain_merge(entry_module_idx) {
        return ReducedEntriesAction::Apply;
      }
      if self.dynamic_entry_supports_namespace_extraction(entry_module_idx) {
        return ReducedEntriesAction::ApplyWithNamespaceExtraction {
          entry_chunk_idx,
          entry_module_idx,
        };
      }
      return ReducedEntriesAction::Avoid;
    }

    ReducedEntriesAction::apply_if(
      !chunk.is_async_entry()
        && !matches!(chunk.preserve_entry_signature, Some(PreserveEntrySignatures::Strict)),
    )
  }

  /// If all the used exports of the dynamic import are statically known and actual exports
  /// of the module, code will never observe the extra exports and we can avoid the extra
  /// indirection of exporting a synthetic namespace.
  fn dynamic_entry_partial_usage_allows_plain_merge(&self, entry_module_idx: ModuleIdx) -> bool {
    let Some(DynamicImportExportsUsage::Partial(used)) =
      self.link_output.dynamic_import_exports_usage_map.get(&entry_module_idx)
    else {
      return false;
    };
    let resolved_exports = &self.link_output.metas[entry_module_idx].resolved_exports;
    used.iter().all(|name| resolved_exports.contains_key(name))
  }

  /// Whether `import()` of this dynamic entry can be preserved by exporting the
  /// entry's namespace object from its chunk and rewriting every dynamic importer to
  /// `.then((n) => n.<ns>)` (see `rewrite_dynamic_import_for_merged_entry`).
  fn dynamic_entry_supports_namespace_extraction(&self, entry_module_idx: ModuleIdx) -> bool {
    if self.link_output.module_table[entry_module_idx].as_normal().is_none() {
      return false;
    }

    // `import()` assimilates a namespace that carries a callable `then`. The extraction callback
    // would then get whatever that `then` returns, not the namespace. Only the export name can do
    // this. A local symbol named `then` that leaves under an alias is harmless, because the alias
    // is what lands in the namespace.
    //
    // No other export name can become `then`. The minified generator skips it, and
    // `ConflictResolver` reserves it up front. So an internal export named `then` deconflicts to
    // `then$1`. See `THENABLE_HAZARD_EXPORT_NAME` in compute_cross_chunk_links.rs.
    if self.link_output.metas[entry_module_idx]
      .resolved_exports
      .keys()
      .any(|name| name.as_str() == "then")
    {
      return false;
    }

    // If the entry has dynamic exports through `export * from`, it _might_
    // have a `then` export.
    !self.link_output.metas[entry_module_idx].has_dynamic_exports
  }

  /// Generates a definition for a fake module namespace for the inlined module, which is then
  /// exported. Dynamic importers of this atom from this chunk can then do `.then((n) => n.<ns>)`
  /// to get that namespace.
  fn apply_dynamic_entry_namespace_extractions(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    namespace_extractions: &FxHashSet<(ChunkIdx, ModuleIdx)>,
    used_symbol_refs_builder: &mut UsedSymbolRefsBuilder,
  ) {
    if namespace_extractions.is_empty() {
      return;
    }

    for &(_, entry_module_idx) in namespace_extractions {
      self.narrow_namespace_stmt_to_used_symbols(entry_module_idx, used_symbol_refs_builder);
    }

    self.replay_link_stage_inclusion(used_symbol_refs_builder, |context| {
      let mut needs_export_all_helper = false;
      for &(entry_chunk_idx, entry_module_idx) in namespace_extractions {
        chunk_graph
          .common_chunk_exported_facade_chunk_namespace
          .entry(entry_chunk_idx)
          .or_default()
          .insert(entry_module_idx);

        if include_simulated_facade_namespace(context, entry_module_idx) {
          chunk_graph.chunk_table[entry_chunk_idx]
            .depended_runtime_helper
            .insert(RuntimeHelper::ExportAll);
          needs_export_all_helper = true;
        }
      }
      needs_export_all_helper
    });
  }

  fn is_runtime_only_atom(&self, atom: &ChunkAtom) -> bool {
    atom.modules.len() == 1 && atom.modules[0] == self.link_output.runtime.id()
  }

  fn removed_entries_are_dynamic_entry_modules(
    atom: &ChunkAtom,
    original_dependent_entries: &BitSet,
    dependent_entries: &BitSet,
    dynamic_entry_modules_by_entry: &[Option<ModuleIdx>],
  ) -> bool {
    let mut has_removed_dynamic_entry = false;
    for removed_entry_idx in
      original_dependent_entries.index_of_one().filter(|idx| !dependent_entries.has_bit(*idx))
    {
      let Some(dynamic_entry_module_idx) =
        dynamic_entry_modules_by_entry.get(removed_entry_idx as usize).copied().flatten()
      else {
        return false;
      };
      has_removed_dynamic_entry = true;
      if !atom.modules.contains(&dynamic_entry_module_idx) {
        return false;
      }
    }
    has_removed_dynamic_entry
  }

  fn compute_module_to_atom_idx(&self, atoms: &[ChunkAtom]) -> IndexVec<ModuleIdx, Option<usize>> {
    let mut module_to_atom_idx = index_vec![None; self.link_output.module_table.modules.len()];
    for (atom_idx, atom) in atoms.iter().enumerate() {
      for &module_idx in &atom.modules {
        module_to_atom_idx[module_idx] = Some(atom_idx);
      }
    }
    module_to_atom_idx
  }

  /// The entry module behind each entry bit, in the bit order `determine_reachable_modules_for_entry`
  /// assigns (`link_output.entries` flattened, one bit per entry point).
  fn flattened_entry_modules(&self) -> Vec<ModuleIdx> {
    self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| std::iter::repeat_n(idx, entries.len()))
      .collect()
  }

  // See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
  //
  // The two graphs serve the pass's two consumers, whose safe approximations point in opposite
  // directions. `reachability` must only contain edges the emitted chunks really have: its
  // entry-export service edges are liveness-gated and attached only when the entry module is
  // hosted by its own entry chunk, because emission hangs those imports on the entry (facade)
  // chunk, not on whichever shared chunk hosts the module. `cycle` must contain every edge the
  // emitted chunks might have: its service edges are attached to the hosting atom regardless,
  // with no liveness gate — `used_symbol_refs_builder` still grows after this pass
  // (namespace-extraction and facade-elimination replays), so an export dead here can be live at
  // emission.
  //
  // Hosting-atom attribution over-approximates rather than misses only because of the facade edges
  // `ReducedChunkGraph` adds: the facade depends on the host, which is where
  // these edges land, so anything emission hangs on the facade stays reachable as facade -> host ->
  // target. Before rolldown#10734 this rested on facades having zero static in-degree, which a
  // fold moving an atom into a facade broke — re-derive it before attributing them elsewhere.
  //
  // The price of the ungated edges is that a dead entry re-export can conservatively veto a fold
  // that would have been acyclic — a missed optimization, accepted over trusting a liveness
  // snapshot the replays may outgrow. The liveness-growth immunity is scoped to service edges:
  // the base prediction's ambiguous branch (`referenced_symbol_owners`) still reads decision-time
  // statement inclusion, a narrower pre-existing skew the replays can also outgrow.
  fn compute_atom_dependencies(
    &self,
    atoms: &[ChunkAtom],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
    flattened_entry_modules: &[ModuleIdx],
    used_symbol_refs_builder: &UsedSymbolRefsBuilder,
  ) -> AtomDependencyGraphs {
    let strict_execution_order = self.options.is_strict_execution_order_enabled();

    let mut reachability = Vec::with_capacity(atoms.len());
    let mut cycle = Vec::with_capacity(atoms.len());
    for (atom_idx, atom) in atoms.iter().enumerate() {
      let mut reachability_deps = FxHashSet::default();
      let mut cycle_deps = FxHashSet::default();
      let add = |dep_module_idx: ModuleIdx, dependencies: &mut FxHashSet<usize>| {
        if let Some(dep_atom_idx) = module_to_atom_idx[dep_module_idx]
          && dep_atom_idx != atom_idx
        {
          dependencies.insert(dep_atom_idx);
        }
      };
      for &module_idx in &atom.modules {
        if strict_execution_order {
          // Strict lowering can turn linked import records back into `init_*` imports.
          for &dep_module_idx in &self.link_output.metas[module_idx].dependencies {
            add(dep_module_idx, &mut reachability_deps);
            add(dep_module_idx, &mut cycle_deps);
          }
          continue;
        }
        for dep_module_idx in self.predicted_static_import_targets(module_idx) {
          add(dep_module_idx, &mut reachability_deps);
          add(dep_module_idx, &mut cycle_deps);
        }

        let mut service_targets = vec![];
        self.entry_export_service_targets(
          module_idx,
          used_symbol_refs_builder.view(),
          true,
          &mut service_targets,
        );
        for dep_module_idx in service_targets {
          add(dep_module_idx, &mut cycle_deps);
        }

        let entry_hosted_by_own_chunk = atom.dependent_entries.bit_count() == 1
          && atom.dependent_entries.index_of_one().next().is_some_and(|entry_bit| {
            flattened_entry_modules.get(entry_bit as usize) == Some(&module_idx)
          });
        if entry_hosted_by_own_chunk {
          let mut service_targets = vec![];
          self.entry_export_service_targets(
            module_idx,
            used_symbol_refs_builder.view(),
            false,
            &mut service_targets,
          );
          for dep_module_idx in service_targets {
            add(dep_module_idx, &mut reachability_deps);
          }
        }
      }
      reachability.push(reachability_deps.into_iter().collect());
      cycle.push(cycle_deps.into_iter().collect());
    }
    AtomDependencyGraphs { reachability, cycle }
  }

  /// Returns the targets that `compute_cross_chunk_links` will import for this module. Transitive
  /// side-effect dependencies behind a `sideEffects: false` barrel are retained only when an
  /// included symbol reference also requires them.
  pub(super) fn predicted_static_import_targets(&self, module_idx: ModuleIdx) -> Vec<ModuleIdx> {
    let meta = &self.link_output.metas[module_idx];
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return meta.load_dependencies.iter().copied().collect();
    };

    // A side-effectful runtime dependency has no import record.
    let side_effectful_runtime_idx =
      meta.has_side_effectful_runtime_dep.then(|| self.link_output.runtime.id());

    let direct_record_targets: FxHashSet<ModuleIdx> = module
      .import_records
      .iter()
      .filter(|rec| rec.kind != ImportKind::DynamicImport)
      .filter_map(|rec| rec.resolved_module)
      .collect();

    let mut ambiguous = vec![];
    let mut targets = Vec::with_capacity(meta.load_dependencies.len());
    for &dep_module_idx in &meta.load_dependencies {
      if !self.link_output.module_table[dep_module_idx].side_effects().has_side_effects()
        || side_effectful_runtime_idx == Some(dep_module_idx)
        || direct_record_targets.contains(&dep_module_idx)
      {
        targets.push(dep_module_idx);
      } else {
        ambiguous.push(dep_module_idx);
      }
    }
    if !ambiguous.is_empty() {
      let symbol_owners = self.referenced_symbol_owners(module_idx);
      targets.extend(ambiguous.into_iter().filter(|dep_idx| symbol_owners.contains(dep_idx)));
    }
    targets
  }

  /// An entry chunk imports the canonical symbol of every live resolved export it serves
  /// (`register_entry_export_depended_symbols`), even when no included statement of the entry
  /// references it — a re-export the entry never uses still becomes a real import of the owner's
  /// chunk. `load_dependencies` only carries the narrowed `referenced_symbols_by_entry_point_chunk`
  /// set, so these targets are derived separately from the full `resolved_exports`.
  ///
  /// With `assume_all_live: false` the liveness gate mirrors emission exactly: a dead export
  /// produces no import. `assume_all_live: true` skips the gate for consumers that need a stable
  /// over-approximation — `used_symbol_refs_builder` keeps growing after the fold's decisions, so a
  /// dead-here export can still emit an import later.
  pub(super) fn entry_export_service_targets(
    &self,
    module_idx: ModuleIdx,
    used_symbol_refs_view: UsedSymbolRefsView<'_>,
    assume_all_live: bool,
    targets: &mut Vec<ModuleIdx>,
  ) {
    if !self.link_output.entries.contains_key(&module_idx) {
      return;
    }
    let meta = &self.link_output.metas[module_idx];
    if matches!(meta.wrap_kind(), WrapKind::Cjs) {
      // A CJS entry is consumed through its wrapper, not through per-export symbols.
      return;
    }
    let symbol_db = &self.link_output.symbol_db;
    for resolved_export in meta.resolved_exports.values() {
      if resolved_export.came_from_commonjs {
        continue;
      }
      let served = symbol_db.canonical_ref_resolving_namespace(resolved_export.symbol_ref);
      let is_live = assume_all_live
        || if let Some(owner) = self.link_output.module_table[served.owner].as_normal()
          && owner.namespace_object_ref == served
        {
          self.link_output.metas[served.owner].namespace_included
        } else {
          used_symbol_refs_view.contains(&served)
        };
      if is_live {
        targets.push(served.owner);
      }
    }
    // A re-export of a wrapped ESM module additionally imports the module's `init_*` wrapper.
    for init in collect_entry_reexported_wrapper_inits(
      module_idx,
      meta,
      &self.link_output.metas,
      &self.link_output.module_table.modules,
      symbol_db,
      None,
    ) {
      if assume_all_live || used_symbol_refs_view.contains(&init.wrapper_ref) {
        targets.push(init.wrapper_ref.owner);
      }
    }
  }

  fn referenced_symbol_owners(&self, module_idx: ModuleIdx) -> FxHashSet<ModuleIdx> {
    let meta = &self.link_output.metas[module_idx];
    let mut owners = FxHashSet::default();
    let note = |symbol_ref: SymbolRef, owners: &mut FxHashSet<ModuleIdx>| {
      let canonical_ref = self.link_output.symbol_db.canonical_ref_for(symbol_ref);
      owners.insert(canonical_ref.owner);
      if let Some(ns) = &self.link_output.symbol_db.get(canonical_ref).namespace_alias {
        owners.insert(ns.namespace_ref.owner);
      }
    };
    for (symbol_ref, _) in &meta.referenced_symbols_by_entry_point_chunk {
      note(*symbol_ref, &mut owners);
    }
    for (stmt_idx, stmt_info) in self.link_output.stmt_infos[module_idx].iter_enumerated() {
      if !meta.stmt_info_included.has_bit(stmt_idx) {
        continue;
      }
      for reference_ref in &stmt_info.referenced_symbols {
        match reference_ref {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => note(*symbol_ref, &mut owners),
          SymbolOrMemberExprRef::MemberExpr(member_expr) => {
            if let Some(symbol_ref) =
              member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs)
            {
              note(symbol_ref, &mut owners);
            }
          }
        }
      }
    }
    owners
  }

  /// The execution order of every module with a side effect of its own, `None` for the others.
  /// Only such a module can observe, or be observed through, a change of execution order: a side
  /// effect a module merely reaches through an import runs at the imported module's own position,
  /// so where the importer runs does not matter. Both passes that move modules into another
  /// chunk's body read this table.
  pub(super) fn own_side_effect_exec_orders(&self) -> IndexVec<ModuleIdx, Option<u32>> {
    self
      .link_output
      .module_table
      .modules
      .iter()
      .map(|module| self.module_has_own_side_effects(module.idx()).then(|| module.exec_order()))
      .collect()
  }

  fn module_has_own_side_effects(&self, module_idx: ModuleIdx) -> bool {
    let module = &self.link_output.module_table[module_idx];
    match module.side_effects() {
      DeterminedSideEffects::UserDefined(user_defined) => *user_defined,
      DeterminedSideEffects::NoTreeshake => true,
      DeterminedSideEffects::Analyzed(false) => false,
      // The analyzed verdict is propagated from the module's dependencies, so an importer of an
      // effectful module carries it without an effectful statement of its own.
      DeterminedSideEffects::Analyzed(true) => {
        let Some(normal_module) = module.as_normal() else {
          return true;
        };
        let meta = &self.link_output.metas[module_idx];
        self.link_output.stmt_infos[module_idx].iter_enumerated().any(|(stmt_idx, stmt_info)| {
          meta.stmt_info_included.has_bit(stmt_idx)
            && stmt_info.eval_flags.has_side_effect_for_tree_shaking()
            && !Self::is_plain_esm_import(stmt_info, normal_module)
        })
      }
    }
  }

  /// An `import` or `export … from` declaration runs its target at the target's own position, so
  /// it is no side effect of the module that holds it. A re-export that merges dynamic exports
  /// keeps a `__reExport` call of its own, and a `require` runs its target in place.
  fn is_plain_esm_import(stmt_info: &StmtInfo, module: &NormalModule) -> bool {
    !stmt_info.import_records.is_empty()
      && !stmt_info.meta.contains(StmtInfoMeta::ReExportDynamicExports)
      && stmt_info
        .import_records
        .iter()
        .all(|&record_idx| module.import_records[record_idx].kind == ImportKind::Import)
  }

  fn analyze_dynamic_entries(
    &self,
    index_splitting_info: &IndexSplittingInfo,
    entries_len: u32,
  ) -> DynamicEntryAnalysis {
    let entries_count = entries_len as usize;
    let mut dynamic_entry_indices = vec![];
    let mut dynamic_entry_modules_by_entry = vec![None; entries_count];
    let mut dynamic_imports_by_entry = vec![BitSet::new(entries_len); entries_count];
    let mut dynamically_dependent_entries_by_dynamic_entry =
      vec![BitSet::new(entries_len); entries_count];

    for (dynamic_entry_idx, (dynamic_entry_module_idx, entry_point)) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| entries.iter().map(move |entry| (idx, entry)))
      .enumerate()
    {
      if !entry_point.kind.is_dynamic_import() {
        continue;
      }

      dynamic_entry_indices.push(dynamic_entry_idx);
      dynamic_entry_modules_by_entry[dynamic_entry_idx] = Some(dynamic_entry_module_idx);
      let dynamic_entry_bit: u32 =
        dynamic_entry_idx.try_into().expect("Too many entries, u32 overflowed.");

      for (importer_idx, stmt_info_idx, _node_id, import_record_idx) in
        &entry_point.related_stmt_infos
      {
        if !self.is_included_dynamic_import_record(
          *importer_idx,
          *stmt_info_idx,
          *import_record_idx,
          dynamic_entry_module_idx,
        ) {
          continue;
        }

        for importer_entry_idx in index_splitting_info[*importer_idx].bits.index_of_one() {
          dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx]
            .set_bit(importer_entry_idx);
          dynamic_imports_by_entry[importer_entry_idx as usize].set_bit(dynamic_entry_bit);
        }
      }
    }

    DynamicEntryAnalysis {
      dynamic_entry_indices,
      dynamic_entry_modules_by_entry,
      dynamic_imports_by_entry,
      dynamically_dependent_entries_by_dynamic_entry,
    }
  }

  fn is_included_dynamic_import_record(
    &self,
    importer_idx: ModuleIdx,
    stmt_info_idx: StmtInfoIdx,
    import_record_idx: ImportRecordIdx,
    dynamic_entry_module_idx: ModuleIdx,
  ) -> bool {
    if !self.link_output.metas[importer_idx].stmt_info_included.has_bit(stmt_info_idx) {
      return false;
    }

    let Some(importer) = self.link_output.module_table[importer_idx].as_normal() else {
      return false;
    };
    let Some(import_record) = importer.import_records.get(import_record_idx) else {
      return false;
    };

    import_record.kind == ImportKind::DynamicImport
      && import_record.resolved_module == Some(dynamic_entry_module_idx)
      && !import_record.meta.contains(ImportRecordMeta::DeadDynamicImport)
  }

  fn group_modules_by_dependent_entries(
    &self,
    index_splitting_info: &IndexSplittingInfo,
  ) -> Vec<ChunkAtom> {
    let mut atoms = vec![];
    let mut bits_to_atom_idx = FxHashMap::default();
    for module_idx in &self.link_output.sorted_modules {
      let Some(normal_module) = self.link_output.module_table[*module_idx].as_normal() else {
        continue;
      };
      if !self.link_output.metas[normal_module.idx].is_included {
        continue;
      }

      let bits = &index_splitting_info[normal_module.idx].bits;
      if bits.is_empty() {
        continue;
      }

      let atom_idx = match bits_to_atom_idx.entry(bits.clone()) {
        std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
        std::collections::hash_map::Entry::Vacant(vacant) => {
          let atom_idx = atoms.len();
          atoms.push(ChunkAtom { modules: vec![], dependent_entries: bits.clone() });
          *vacant.insert(atom_idx)
        }
      };
      atoms[atom_idx].modules.push(normal_module.idx);
    }
    atoms
  }

  fn compute_static_dependency_atoms_by_entry(
    &self,
    entries_count: usize,
    atoms: &[ChunkAtom],
    atom_dependencies: &[Vec<usize>],
    module_to_atom_idx: &IndexVec<ModuleIdx, Option<usize>>,
  ) -> Vec<BitSet> {
    let atom_count: u32 = atoms.len().try_into().expect("Too many atoms, u32 overflowed.");
    let mut static_dependency_atoms_by_entry = vec![BitSet::new(atom_count); entries_count];

    if self.options.is_strict_execution_order_enabled() {
      // Conservative strict edges are safe for cycle detection, but cannot prove an atom is loaded.
      // Keep the `load_dependencies` reachability recorded in the dependent-entry bits.
      for (atom_idx, atom) in atoms.iter().enumerate() {
        let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
        for entry_idx in atom.dependent_entries.index_of_one() {
          static_dependency_atoms_by_entry[entry_idx as usize].set_bit(atom_bit);
        }
      }
      return static_dependency_atoms_by_entry;
    }

    for (entry_module_idx, loaded_atoms) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(&idx, entries)| std::iter::repeat_n(idx, entries.len()))
      .zip(static_dependency_atoms_by_entry.iter_mut())
    {
      let Some(entry_atom_idx) = module_to_atom_idx[entry_module_idx] else {
        continue;
      };
      let mut stack = vec![entry_atom_idx];
      while let Some(atom_idx) = stack.pop() {
        let atom_bit: u32 = atom_idx.try_into().expect("Too many atoms, u32 overflowed.");
        if loaded_atoms.has_bit(atom_bit) {
          continue;
        }
        loaded_atoms.set_bit(atom_bit);
        stack.extend(atom_dependencies[atom_idx].iter().copied());
      }
    }
    static_dependency_atoms_by_entry
  }

  fn compute_already_loaded_atoms_by_entry(
    static_dependency_atoms_by_entry: &[BitSet],
    mut dynamically_dependent_entries_by_dynamic_entry: Vec<BitSet>,
    dynamic_imports_by_entry: &[BitSet],
    dynamic_entry_indices: &[usize],
    atom_count: usize,
  ) -> Vec<BitSet> {
    let entries_count = static_dependency_atoms_by_entry.len();
    let atom_count: u32 = atom_count.try_into().expect("Too many atoms, u32 overflowed.");
    let mut is_dynamic_entry = vec![false; entries_count];
    for &dynamic_entry_idx in dynamic_entry_indices {
      is_dynamic_entry[dynamic_entry_idx] = true;
    }

    let mut already_loaded_atoms_by_entry = is_dynamic_entry
      .iter()
      .map(|is_dynamic| if *is_dynamic { BitSet::all(atom_count) } else { BitSet::new(atom_count) })
      .collect::<Vec<_>>();

    let mut queued = vec![false; entries_count];
    let mut queue = VecDeque::new();
    for &dynamic_entry_idx in dynamic_entry_indices {
      if !dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx].is_empty() {
        queued[dynamic_entry_idx] = true;
        queue.push_back(dynamic_entry_idx);
      }
    }

    while let Some(dynamic_entry_idx) = queue.pop_front() {
      queued[dynamic_entry_idx] = false;
      let known_loaded_atoms = already_loaded_atoms_by_entry[dynamic_entry_idx].clone();
      let mut updated_loaded_atoms = known_loaded_atoms.clone();

      for importer_entry_idx in
        dynamically_dependent_entries_by_dynamic_entry[dynamic_entry_idx].index_of_one()
      {
        let importer_entry_idx = importer_entry_idx as usize;
        let mut importer_loaded_atoms =
          static_dependency_atoms_by_entry[importer_entry_idx].clone();
        importer_loaded_atoms.union(&already_loaded_atoms_by_entry[importer_entry_idx]);
        updated_loaded_atoms.intersect(&importer_loaded_atoms);
      }

      if updated_loaded_atoms == known_loaded_atoms {
        continue;
      }

      already_loaded_atoms_by_entry[dynamic_entry_idx] = updated_loaded_atoms;
      let dynamic_entry_bit: u32 =
        dynamic_entry_idx.try_into().expect("Too many entries, u32 overflowed.");
      for next_dynamic_entry_idx in dynamic_imports_by_entry[dynamic_entry_idx].index_of_one() {
        let next_dynamic_entry_idx = next_dynamic_entry_idx as usize;
        dynamically_dependent_entries_by_dynamic_entry[next_dynamic_entry_idx]
          .set_bit(dynamic_entry_bit);
        if !queued[next_dynamic_entry_idx] {
          queued[next_dynamic_entry_idx] = true;
          queue.push_back(next_dynamic_entry_idx);
        }
      }
    }

    already_loaded_atoms_by_entry
  }
}
