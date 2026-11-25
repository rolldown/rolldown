use oxc_index::IndexVec;
use rolldown_common::{ModuleIdx, ModuleTable};
use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use super::fold_execution_order::{EntryOrder, EntryOrders};

/// The chunk graph that the current dependent-entry bits of the atoms produce.
/// `optimize_dynamic_entry_bits` moves atoms to reduced bits, and the graph follows each move.
/// Atoms with equal bits form one node (a chunk). A node depends on another node when one of its
/// atoms depends on an atom of the other node. A node also depends on another node through a
/// facade edge (see `facade_edge`).
///
/// Every edge that a move adds starts or ends at the node that receives the atom (see
/// `reaches_itself`). So while the graph is acyclic, a move closes a cycle exactly when a path
/// leads from that node back to itself. The check is one search from that node. A move removes
/// only edges of the node that the atom leaves. So while the graph has a cycle, only a move out of
/// a node that lies on every cycle can make the graph acyclic. Only such a move gets a full check.
/// See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
pub(super) struct ReducedChunkGraph<'a> {
  atom_dependencies: &'a [Vec<usize>],
  atom_dependents: Vec<Vec<usize>>,
  host_atom_by_entry_bit: &'a [Option<usize>],
  hosted_entry_bits_by_atom: Vec<SmallVec<[usize; 1]>>,
  node_by_bits: FxHashMap<BitSet, usize>,
  node_by_atom: Vec<usize>,
  /// A node without atoms is not a chunk. It has no edges.
  atoms_by_node: Vec<Vec<usize>>,
  bits_by_node: Vec<BitSet>,
  /// The entry bit of a node whose bits hold only that bit. Such a node is that entry's chunk.
  facade_entry_bit_by_node: Vec<Option<usize>>,
  facade_node_by_entry_bit: Vec<Option<usize>>,
  edges: NodeEdges,
  is_acyclic: bool,
  /// While the graph has a cycle, the nodes of its only strongly connected component with more
  /// than one node. If the graph has two or more such components, this set is empty. Each cycle
  /// lies in one such component, and a move removes only edges of the node that the atom leaves.
  /// So a move out of a node outside this set keeps a cycle.
  cyclic_component: FxHashSet<usize>,
  /// Per node, the `mark` of the last search that visited it.
  marks: Vec<u32>,
  mark: u32,
  /// Set by `check_execution_order`.
  execution_order: Option<AtomExecutionOrders<'a>>,
  entry_orders: Option<EntryOrders<'a>>,
  /// The atoms that import each external of `AtomExecutionOrders::externals`.
  external_importers: FxHashMap<ModuleIdx, Vec<usize>>,
  /// Per node, the externals of `AtomExecutionOrders::externals` that its atoms import, with the
  /// number of its atoms that import each.
  externals_by_node: Vec<FxHashMap<ModuleIdx, u32>>,
  summaries: NodeOrderSummaries,
  imported: NodeSet,
}

/// A set of nodes that a check fills and reads many times, kept between checks to save the
/// allocation. `clear` costs nothing: a node is in the set when its mark is the current one.
#[derive(Default)]
struct NodeSet {
  nodes: Vec<usize>,
  marks: Vec<u32>,
  mark: u32,
}

impl NodeSet {
  fn clear(&mut self, node_count: usize) {
    self.nodes.clear();
    self.marks.resize(node_count, 0);
    if self.mark == u32::MAX {
      self.marks.fill(0);
      self.mark = 0;
    }
    self.mark += 1;
  }

  fn insert(&mut self, node: usize) -> bool {
    if self.marks[node] == self.mark {
      return false;
    }
    self.marks[node] = self.mark;
    self.nodes.push(node);
    true
  }

  fn contains(&self, node: usize) -> bool {
    self.marks.get(node).is_some_and(|&mark| mark == self.mark)
  }
}

/// What the execution-order check reads. See `ReducedChunkGraph::check_execution_order`.
pub(super) struct AtomExecutionOrders<'a> {
  /// The under-approximated atom edges: imports the emitted chunks are certain to have.
  pub(super) certain_dependencies: &'a [Vec<usize>],
  /// The modules of each atom.
  pub(super) modules: &'a [Vec<ModuleIdx>],
  /// The modules of each atom whose position matters.
  pub(super) order_sensitive: &'a [Vec<ModuleIdx>],
  /// The externals whose position matters that each atom imports.
  pub(super) externals: &'a [Vec<ModuleIdx>],
  pub(super) module_to_atom_idx: &'a IndexVec<ModuleIdx, Option<usize>>,
  pub(super) module_table: &'a ModuleTable,
}

/// What the execution-order check keeps about the atoms that a node held before the move being
/// checked, so that a move costs about as much as its own atoms, not as all the atoms of the
/// receiving node. The atoms of a node other than the receiving one do not change during a check,
/// except in the nodes that the moved atoms left, whose spans and runs are dropped after each
/// check.
#[derive(Default)]
struct NodeOrderSummaries {
  /// Per node, its modules that matter, sorted by `exec_order`.
  by_exec_order: Vec<Vec<(u32, ModuleIdx)>>,
  /// The nodes whose body can run two modules that matter in an order that an entry does not run
  /// them in. A group move checks each moved atom against the atoms that were there, not against
  /// the other moved atoms. Elsewhere, the body runs the modules of `by_exec_order` in the order of
  /// each entry, so a moved module only needs to be checked against its neighbors there.
  unordered: FxHashSet<usize>,
  /// Per node and entry bit, the positions of the node's modules in the entry.
  spans: Vec<FxHashMap<usize, Span>>,
  /// Per node and entry bit, a range of indices into `EntryOrder::order_sensitive` whose modules
  /// all belong to the node.
  owned_runs: FxHashMap<usize, FxHashMap<usize, (usize, usize)>>,
}

#[derive(Clone, Copy, Default)]
struct Span {
  /// The first position of a module.
  first: Option<u32>,
  /// The first and the last position of a module that matters.
  first_order_sensitive: Option<u32>,
  last_order_sensitive: Option<u32>,
}

impl Span {
  fn add_atom(&mut self, orders: &AtomExecutionOrders<'_>, entry: &EntryOrder, atom_idx: usize) {
    let positions = |modules: &[ModuleIdx]| -> Vec<u32> {
      modules.iter().filter_map(|module_idx| entry.position.get(module_idx).copied()).collect()
    };
    if let Some(&first) = positions(&orders.modules[atom_idx]).iter().min() {
      self.first = Some(self.first.map_or(first, |current| current.min(first)));
    }
    let order_sensitive = positions(&orders.order_sensitive[atom_idx]);
    if let (Some(&first), Some(&last)) =
      (order_sensitive.iter().min(), order_sensitive.iter().max())
    {
      self.first_order_sensitive =
        Some(self.first_order_sensitive.map_or(first, |current| current.min(first)));
      self.last_order_sensitive =
        Some(self.last_order_sensitive.map_or(last, |current| current.max(last)));
    }
  }
}

/// An entry that runs the receiving node, with its bit.
type CheckedEntry<'e> = (usize, &'e EntryOrder);

/// What ran before an atom in its chunk before a move: the atoms it is certain to import, directly
/// or not, and the externals that it and those atoms import.
#[derive(Default)]
struct LoadedBefore {
  atoms: FxHashSet<usize>,
  externals: FxHashSet<ModuleIdx>,
}

/// Whether an atom in the body of the receiving node runs after a module or an external that the
/// node imports, directly or not, and that an entry runs after the atom. One that the atom is
/// certain to import ran before it already. That is decided per atom, not per chunk: a chunk the
/// atom imports can hold atoms that other moves put there, and those ran after it. The checked
/// atoms are the moved ones against everything the node imports, and the ones that were there
/// before against what the move adds to the imports.
struct LateImportCheck<'c, 'a> {
  orders: &'c AtomExecutionOrders<'a>,
  entries: &'c [CheckedEntry<'c>],
  checked: &'c [usize],
  /// Per checked atom and entry, the first position of the atom's modules that matter.
  earliest: Vec<Vec<Option<u32>>>,
  loaded_before: Vec<Option<LoadedBefore>>,
  /// Externals that the node imported before the move, which add nothing.
  imported_externals: FxHashSet<ModuleIdx>,
}

impl<'c, 'a> LateImportCheck<'c, 'a> {
  fn new(
    orders: &'c AtomExecutionOrders<'a>,
    entries: &'c [CheckedEntry<'c>],
    checked: &'c [usize],
  ) -> Self {
    let earliest = checked
      .iter()
      .map(|&atom_idx| {
        entries
          .iter()
          .map(|(_, entry)| {
            orders.order_sensitive[atom_idx]
              .iter()
              .filter_map(|module_idx| entry.position.get(module_idx).copied())
              .min()
          })
          .collect()
      })
      .collect();
    let loaded_before = checked.iter().map(|_| None).collect();
    Self {
      orders,
      entries,
      checked,
      earliest,
      loaded_before,
      imported_externals: FxHashSet::default(),
    }
  }

  fn externals_run_after(
    &mut self,
    graph: &ReducedChunkGraph<'_>,
    externals: &FxHashMap<ModuleIdx, u32>,
  ) -> bool {
    if externals.is_empty() {
      return false;
    }
    let orders = self.orders;
    for (checked_idx, &atom_idx) in self.checked.iter().enumerate() {
      for ((_, entry), &earliest) in self.entries.iter().zip(&self.earliest[checked_idx]) {
        let Some(earliest) = earliest else {
          continue;
        };
        let loaded_before = &mut self.loaded_before[checked_idx];
        if externals.keys().any(|external| {
          !self.imported_externals.contains(external)
            && entry.position.get(external).is_some_and(|&position| position > earliest)
            && !loaded_before
              .get_or_insert_with(|| graph.certainly_loaded_before(orders, atom_idx))
              .externals
              .contains(external)
        }) {
          return true;
        }
      }
    }
    false
  }

  /// Checks the modules of the atoms of `node`, which is not the receiving node.
  fn modules_run_after(
    &mut self,
    graph: &ReducedChunkGraph<'_>,
    summaries: &mut NodeOrderSummaries,
    node: usize,
  ) -> bool {
    let orders = self.orders;
    for (checked_idx, &atom_idx) in self.checked.iter().enumerate() {
      for (&(entry_bit, entry), &earliest) in self.entries.iter().zip(&self.earliest[checked_idx]) {
        let Some(earliest) = earliest else {
          continue;
        };
        let last = summaries.span(graph, orders, node, entry_bit, entry, &[]).last_order_sensitive;
        if last.is_none_or(|last| last <= earliest) {
          continue;
        }
        let loaded_before = &mut self.loaded_before[checked_idx];
        for &member in &graph.atoms_by_node[node] {
          if orders.order_sensitive[member].iter().any(|module_idx| {
            entry.position.get(module_idx).is_some_and(|&position| position > earliest)
          }) && !loaded_before
            .get_or_insert_with(|| graph.certainly_loaded_before(orders, atom_idx))
            .atoms
            .contains(&member)
          {
            return true;
          }
        }
      }
    }
    false
  }
}

impl NodeOrderSummaries {
  /// The span of the atoms of `node` other than `moved` in the entry.
  fn span(
    &mut self,
    graph: &ReducedChunkGraph<'_>,
    orders: &AtomExecutionOrders<'_>,
    node: usize,
    entry_bit: usize,
    entry: &EntryOrder,
    moved: &[usize],
  ) -> Span {
    if self.spans.len() <= node {
      self.spans.resize_with(node + 1, FxHashMap::default);
    }
    *self.spans[node].entry(entry_bit).or_insert_with(|| {
      let mut span = Span::default();
      for &atom_idx in graph.atoms_by_node[node].iter().filter(|atom_idx| !moved.contains(atom_idx))
      {
        span.add_atom(orders, entry, atom_idx);
      }
      span
    })
  }

  fn order_sensitive_of(&self, node: usize) -> &[(u32, ModuleIdx)] {
    self.by_exec_order.get(node).map_or(&[], Vec::as_slice)
  }

  /// Records the move of `moved` from `from_nodes` into `node` after the check accepted it.
  fn commit(
    &mut self,
    orders: &AtomExecutionOrders<'_>,
    entries: &[CheckedEntry<'_>],
    moved: &[usize],
    from_nodes: &[usize],
    node: usize,
  ) {
    let exec_ordered = |atom_idx: usize| {
      orders.order_sensitive[atom_idx]
        .iter()
        .map(|&module_idx| (orders.module_table[module_idx].exec_order(), module_idx))
    };
    for (&atom_idx, &from) in moved.iter().zip(from_nodes) {
      let modules: FxHashSet<(u32, ModuleIdx)> = exec_ordered(atom_idx).collect();
      self.by_exec_order[from].retain(|module| !modules.contains(module));
    }
    if self.by_exec_order.len() <= node {
      self.by_exec_order.resize_with(node + 1, Vec::new);
    }
    let added: Vec<(u32, ModuleIdx)> =
      moved.iter().flat_map(|&atom_idx| exec_ordered(atom_idx)).collect();
    let body = &mut self.by_exec_order[node];
    for &module in &added {
      let index = body.partition_point(|&other| other < module);
      body.insert(index, module);
    }
    if moved.len() > 1
      && entries.iter().any(|(_, entry)| {
        let mut positions: Vec<(u32, u32)> = added
          .iter()
          .filter_map(|&(exec_order, module_idx)| {
            Some((exec_order, *entry.position.get(&module_idx)?))
          })
          .collect();
        positions.sort_unstable();
        positions.windows(2).any(|pair| pair[0].1 > pair[1].1)
      })
    {
      self.unordered.insert(node);
    }
    if let Some(spans) = self.spans.get_mut(node).filter(|spans| !spans.is_empty()) {
      let entry_bits: FxHashSet<usize> = entries.iter().map(|&(entry_bit, _)| entry_bit).collect();
      for &(entry_bit, entry) in entries {
        if let Some(span) = spans.get_mut(&entry_bit) {
          for &atom_idx in moved {
            span.add_atom(orders, entry, atom_idx);
          }
        }
      }
      // Spans of entries that the check skipped do not hold the moved atoms.
      spans.retain(|entry_bit, _| entry_bits.contains(entry_bit));
    }
  }

  /// Drops what can be stale in the nodes that the moved atoms left: a check reads them without
  /// those atoms, and an accepted move takes the atoms out.
  fn forget(&mut self, from_nodes: &[usize], accepted: bool) {
    for from in from_nodes {
      if let Some(spans) = self.spans.get_mut(*from) {
        spans.clear();
      }
      if accepted {
        self.owned_runs.remove(from);
      }
    }
  }
}

/// The nodes that a move touches, seen from one entry that runs the receiving node.
struct MoveInEntry<'m> {
  entry_bit: usize,
  entry: &'m EntryOrder,
  moved: &'m [usize],
  node: usize,
  /// The nodes that the receiving node imports, directly or not.
  imported: &'m NodeSet,
}

impl<'a> ReducedChunkGraph<'a> {
  /// `atom_bits` holds the initial dependent-entry bits of each atom. No two atoms have equal bits.
  pub(super) fn new<'b>(
    atom_bits: impl IntoIterator<Item = &'b BitSet>,
    atom_dependencies: &'a [Vec<usize>],
    host_atom_by_entry_bit: &'a [Option<usize>],
  ) -> Self {
    let atom_count = atom_dependencies.len();
    let mut graph = Self {
      atom_dependencies,
      atom_dependents: vec![vec![]; atom_count],
      host_atom_by_entry_bit,
      hosted_entry_bits_by_atom: vec![SmallVec::new(); atom_count],
      node_by_bits: FxHashMap::default(),
      node_by_atom: Vec::with_capacity(atom_count),
      atoms_by_node: Vec::with_capacity(atom_count),
      bits_by_node: Vec::with_capacity(atom_count),
      facade_entry_bit_by_node: Vec::with_capacity(atom_count),
      facade_node_by_entry_bit: vec![None; host_atom_by_entry_bit.len()],
      edges: NodeEdges::default(),
      is_acyclic: false,
      cyclic_component: FxHashSet::default(),
      marks: Vec::with_capacity(atom_count),
      mark: 0,
      execution_order: None,
      entry_orders: None,
      external_importers: FxHashMap::default(),
      externals_by_node: Vec::with_capacity(atom_count),
      summaries: NodeOrderSummaries::default(),
      imported: NodeSet::default(),
    };
    // Each atom starts as the only atom of its own node, so node and atom indices match.
    for bits in atom_bits {
      let node = graph.add_node(bits);
      debug_assert_eq!(node, graph.node_by_atom.len(), "atoms must have different bits");
      graph.node_by_atom.push(node);
      graph.atoms_by_node[node].push(node);
    }
    debug_assert_eq!(graph.node_by_atom.len(), atom_count);
    for (atom_idx, dependencies) in atom_dependencies.iter().enumerate() {
      for &dependency in dependencies {
        graph.atom_dependents[dependency].push(atom_idx);
        graph.edges.add(atom_idx, dependency);
      }
    }
    for (entry_bit, host_atom_idx) in host_atom_by_entry_bit.iter().enumerate() {
      if let Some(host_atom_idx) = *host_atom_idx {
        graph.hosted_entry_bits_by_atom[host_atom_idx].push(entry_bit);
      }
      if let Some((facade, host)) = graph.facade_edge(entry_bit) {
        graph.edges.add(facade, host);
      }
    }
    graph.is_acyclic = !graph.has_cycle();
    if !graph.is_acyclic {
      graph.cyclic_component = graph.only_cyclic_component();
    }
    graph
  }

  /// Also rejects a move that changes the order of two modules whose position matters (see
  /// `breaks_execution_order`).
  pub(super) fn check_execution_order(
    &mut self,
    execution_order: AtomExecutionOrders<'a>,
    entry_orders: EntryOrders<'a>,
  ) {
    for (atom_idx, externals) in execution_order.externals.iter().enumerate() {
      for &external in externals {
        self.external_importers.entry(external).or_default().push(atom_idx);
        *self.externals_by_node[self.node_by_atom[atom_idx]].entry(external).or_default() += 1;
      }
    }
    self.summaries.by_exec_order = vec![vec![]; self.atoms_by_node.len()];
    for (atom_idx, order_sensitive) in execution_order.order_sensitive.iter().enumerate() {
      let body = &mut self.summaries.by_exec_order[self.node_by_atom[atom_idx]];
      body.extend(
        order_sensitive
          .iter()
          .map(|&module_idx| (execution_order.module_table[module_idx].exec_order(), module_idx)),
      );
      body.sort_unstable();
    }
    self.execution_order = Some(execution_order);
    self.entry_orders = Some(entry_orders);
  }

  /// Moves the atom to the node of `bits` and returns true if the chunk graph stays acyclic and,
  /// when checked, the execution order holds. Otherwise, it keeps the atom where it was and returns
  /// false.
  pub(super) fn try_move(&mut self, atom_idx: usize, bits: &BitSet) -> bool {
    self.try_move_atoms(&[atom_idx], bits)
  }

  /// Moves all the atoms to the node of `bits` and returns true if the chunk graph stays acyclic and,
  /// when checked, the execution order holds. Otherwise, it keeps every atom where it was and
  /// returns false. Atoms that depend on each other can close a cycle when they move one by one and
  /// stay acyclic when they move together, and atoms can each run late against the others.
  pub(super) fn try_move_atoms(&mut self, atom_idxs: &[usize], bits: &BitSet) -> bool {
    let from_nodes: SmallVec<[usize; 1]> =
      atom_idxs.iter().map(|&atom_idx| self.node_by_atom[atom_idx]).collect();
    // Moves out of two nodes can break cycles in two components, so the shortcut needs one node.
    if !self.is_acyclic
      && from_nodes.iter().all(|&from| from == from_nodes[0])
      && !self.cyclic_component.contains(&from_nodes[0])
    {
      return false;
    }
    let to = match self.node_by_bits.get(bits) {
      Some(&node) => node,
      None => self.add_node(bits),
    };
    for (&atom_idx, &from) in atom_idxs.iter().zip(&from_nodes) {
      debug_assert_ne!(from, to, "the reduced bits must differ from the current bits");
      self.move_atom(atom_idx, from, to);
    }
    // All the atoms go to `to`, so every edge that the moves add still starts or ends at `to`.
    // While the graph has a cycle, the moves can remove every old cycle. They can also close new
    // cycles through `to`. So only a full check is exact. A rejected move leaves
    // `cyclic_component` correct.
    let acyclic = if self.is_acyclic { !self.reaches_itself(to) } else { !self.has_cycle() };
    let accepted = acyclic && !self.breaks_execution_order(atom_idxs, &from_nodes, to);
    if accepted {
      self.is_acyclic = true;
    } else {
      for (&atom_idx, &from) in atom_idxs.iter().zip(&from_nodes).rev() {
        self.move_atom(atom_idx, to, from);
      }
    }
    accepted
  }

  /// Whether the move of `moved` into `node` changes the order in which an entry runs two modules
  /// whose position matters. Each entry that runs `node` is checked with its own order (see
  /// `EntryOrders`). A chunk runs every chunk and external it imports before its body, its body runs
  /// its modules in `exec_order`, and an importer runs it where it first needs one of its modules.
  fn breaks_execution_order(&mut self, moved: &[usize], from_nodes: &[usize], node: usize) -> bool {
    let (Some(mut entry_orders), Some(orders)) =
      (self.entry_orders.take(), self.execution_order.as_ref())
    else {
      return false;
    };
    let mut summaries = std::mem::take(&mut self.summaries);
    let mut imported = std::mem::take(&mut self.imported);
    let entry_bits: Vec<usize> =
      self.bits_by_node[node].index_of_one().map(|bit| bit as usize).collect();
    // An entry that runs two modules of one atom against their `exec_order` runs out of order with
    // or without the move, since an atom stays in one chunk. Its order only counts when the move
    // can keep it.
    let entries: Vec<CheckedEntry<'_>> = entry_bits
      .iter()
      .copied()
      .zip(entry_orders.walk(&entry_bits))
      .filter(|(_, entry)| !entry.runs_an_atom_out_of_order)
      .collect();
    let breaks = self.breaks_order(&mut summaries, &mut imported, orders, &entries, moved, node);
    if !breaks {
      summaries.commit(orders, &entries, moved, from_nodes, node);
    }
    summaries.forget(from_nodes, !breaks);
    drop(entries);
    self.summaries = summaries;
    self.imported = imported;
    self.entry_orders = Some(entry_orders);
    breaks
  }

  fn breaks_order(
    &self,
    summaries: &mut NodeOrderSummaries,
    imported: &mut NodeSet,
    orders: &AtomExecutionOrders<'_>,
    entries: &[CheckedEntry<'_>],
    moved: &[usize],
    node: usize,
  ) -> bool {
    if entries.is_empty() {
      return false;
    }
    let mut late = LateImportCheck::new(orders, entries, moved);
    // The externals that the node imports run before its body too.
    if late.externals_run_after(self, &self.externals_by_node[node]) {
      return true;
    }
    imported.clear(self.atoms_by_node.len());
    let mut stack: Vec<usize> = self.edges.dependencies[node].keys().copied().collect();
    while let Some(next) = stack.pop() {
      if next == node || !imported.insert(next) {
        continue;
      }
      if late.modules_run_after(self, summaries, next)
        || late.externals_run_after(self, &self.externals_by_node[next])
      {
        return true;
      }
      stack.extend(self.edges.dependencies[next].keys().copied());
    }
    if self.staying_run_after_added_imports(summaries, orders, entries, moved, node, imported) {
      return true;
    }
    // A node with one bit is the chunk of that entry, which no importer runs.
    let is_imported_by_entry = self.bits_by_node[node].bit_count() > 1;
    let unordered_staying = summaries.unordered.contains(&node).then(|| self.staying(moved, node));
    entries.iter().any(|&(entry_bit, entry)| {
      let view = MoveInEntry { entry_bit, entry, moved, node, imported };
      let body_reorders = match &unordered_staying {
        Some(staying) => self.body_reorders_against(orders, &view, staying),
        None => self.body_reorders(summaries, orders, &view),
      };
      body_reorders
        || (is_imported_by_entry && self.evaluation_point_crosses(summaries, orders, &view))
    })
  }

  /// The atoms of `node` that were there before the move of `moved`.
  fn staying(&self, moved: &[usize], node: usize) -> Vec<usize> {
    self.atoms_by_node[node].iter().copied().filter(|atom_idx| !moved.contains(atom_idx)).collect()
  }

  /// Whether an atom that was in the node before the move now runs after a module or an external
  /// that only the moved atoms make the node import. In
  /// `chunk_optimization/dynamic_already_loaded_order_runtime`, `m3.js` moves first, and then
  /// `m4.js` and `m2.js`, whose `export *` makes the chunk import the chunk of `m0.js`, which the
  /// entry runs after `m3.js`.
  fn staying_run_after_added_imports(
    &self,
    summaries: &mut NodeOrderSummaries,
    orders: &AtomExecutionOrders<'_>,
    entries: &[CheckedEntry<'_>],
    moved: &[usize],
    node: usize,
    imported: &NodeSet,
  ) -> bool {
    if self.atoms_by_node[node].len() == moved.len() {
      return false;
    }
    let mut moved_externals: FxHashMap<ModuleIdx, u32> = FxHashMap::default();
    for &atom_idx in moved {
      for &external in &orders.externals[atom_idx] {
        *moved_externals.entry(external).or_default() += 1;
      }
    }
    let staying_externals: FxHashSet<ModuleIdx> = self.externals_by_node[node]
      .iter()
      .filter(|&(external, &count)| moved_externals.get(external).copied().unwrap_or(0) < count)
      .map(|(&external, _)| external)
      .collect();
    let added_externals: FxHashMap<ModuleIdx, u32> = moved_externals
      .into_iter()
      .filter(|(external, _)| !staying_externals.contains(external))
      .collect();
    let added_nodes: Vec<usize> = if imported.nodes.is_empty() {
      vec![]
    } else {
      self.imported_by_staying(moved, node).map_or_else(Vec::new, |imported_before| {
        imported.nodes.iter().copied().filter(|next| !imported_before.contains(next)).collect()
      })
    };
    if added_externals.is_empty() && added_nodes.is_empty() {
      return false;
    }
    // A staying module that matters runs at or after the first of them in each entry, so nothing
    // that an entry runs before that can run late against one.
    let first_staying: Vec<Option<u32>> = entries
      .iter()
      .map(|&(entry_bit, entry)| {
        summaries.span(self, orders, node, entry_bit, entry, moved).first_order_sensitive
      })
      .collect();
    let runs_after_first_staying = |external: &ModuleIdx| {
      entries.iter().zip(&first_staying).any(|((_, entry), first)| {
        first.is_some_and(|first| {
          entry.position.get(external).is_some_and(|&position| position > first)
        })
      })
    };
    let externals_can_run_late = |externals: &FxHashMap<ModuleIdx, u32>| {
      externals
        .keys()
        .any(|external| !staying_externals.contains(external) && runs_after_first_staying(external))
    };
    let mut candidates = vec![];
    for &next in &added_nodes {
      let modules_can_run_late =
        entries.iter().zip(&first_staying).any(|(&(entry_bit, entry), first)| {
          first.is_some_and(|first| {
            summaries
              .span(self, orders, next, entry_bit, entry, &[])
              .last_order_sensitive
              .is_some_and(|last| last > first)
          })
        });
      if modules_can_run_late || externals_can_run_late(&self.externals_by_node[next]) {
        candidates.push(next);
      }
    }
    if candidates.is_empty() && !externals_can_run_late(&added_externals) {
      return false;
    }
    let staying = self.staying(moved, node);
    let mut late = LateImportCheck::new(orders, entries, &staying);
    late.imported_externals = staying_externals;
    late.externals_run_after(self, &added_externals)
      || candidates.iter().any(|&next| {
        late.modules_run_after(self, summaries, next)
          || late.externals_run_after(self, &self.externals_by_node[next])
      })
  }

  /// The nodes that the atoms of `node` other than `moved` import, directly or not. The edge count
  /// of a node pair is the number of atom edges between them, plus one for a facade edge. `None`
  /// when they import every node that `node` imports directly, so they import all it imports.
  fn imported_by_staying(&self, moved: &[usize], node: usize) -> Option<FxHashSet<usize>> {
    let mut moved_edges: FxHashMap<usize, u32> = FxHashMap::default();
    for &atom_idx in moved {
      for &dependency in &self.atom_dependencies[atom_idx] {
        let dependency_node = self.node_by_atom[dependency];
        if dependency_node != node {
          *moved_edges.entry(dependency_node).or_default() += 1;
        }
      }
    }
    if let Some((facade, host)) =
      self.facade_entry_bit_by_node[node].and_then(|entry_bit| self.facade_edge(entry_bit))
    {
      debug_assert_eq!(facade, node);
      *moved_edges.entry(host).or_default() += 1;
    }
    let dependencies = &self.edges.dependencies[node];
    // With the same first nodes, the search reaches the same nodes as the one for `imported`.
    if moved_edges
      .iter()
      .all(|(next, &count)| dependencies.get(next).is_some_and(|&all| all > count))
    {
      return None;
    }
    let mut stack: Vec<usize> = dependencies
      .iter()
      .filter(|&(next, &count)| moved_edges.get(next).copied().unwrap_or(0) < count)
      .map(|(&next, _)| next)
      .collect();
    let mut imported_before = FxHashSet::default();
    while let Some(next) = stack.pop() {
      if next == node || !imported_before.insert(next) {
        continue;
      }
      stack.extend(self.edges.dependencies[next].keys().copied());
    }
    Some(imported_before)
  }

  fn certainly_loaded_before(
    &self,
    orders: &AtomExecutionOrders<'_>,
    atom_idx: usize,
  ) -> LoadedBefore {
    let mut loaded = LoadedBefore::default();
    loaded.externals.extend(&orders.externals[atom_idx]);
    let mut stack = orders.certain_dependencies[atom_idx].clone();
    while let Some(next) = stack.pop() {
      if next != atom_idx && loaded.atoms.insert(next) {
        loaded.externals.extend(&orders.externals[next]);
        stack.extend(&orders.certain_dependencies[next]);
      }
    }
    loaded
  }

  /// Whether the body of the node, which runs its modules in `exec_order`, runs a moved module and
  /// a module that was there before in the order that the entry does not run them in. The modules
  /// that were there run in the order of the entry (see `NodeOrderSummaries::unordered`), so each
  /// moved module only needs to fall between its two neighbors in `exec_order`.
  fn body_reorders(
    &self,
    summaries: &NodeOrderSummaries,
    orders: &AtomExecutionOrders<'_>,
    view: &MoveInEntry<'_>,
  ) -> bool {
    let body = summaries.order_sensitive_of(view.node);
    let position = |module_idx: &ModuleIdx| view.entry.position.get(module_idx).copied();
    view.moved.iter().flat_map(|&atom_idx| &orders.order_sensitive[atom_idx]).any(|module_idx| {
      let Some(moved_position) = position(module_idx) else {
        return false;
      };
      let index = body.partition_point(|&(exec_order, _)| {
        exec_order < orders.module_table[*module_idx].exec_order()
      });
      body[..index]
        .iter()
        .rev()
        .find_map(|(_, before)| position(before))
        .is_some_and(|before| before > moved_position)
        || body[index..]
          .iter()
          .find_map(|(_, after)| position(after))
          .is_some_and(|after| after < moved_position)
    })
  }

  /// `body_reorders` for a node whose modules may not run in the order of the entry: each moved
  /// module against each module that was there.
  fn body_reorders_against(
    &self,
    orders: &AtomExecutionOrders<'_>,
    view: &MoveInEntry<'_>,
    staying: &[usize],
  ) -> bool {
    let placed = |atoms: &[usize]| -> Vec<(u32, u32)> {
      atoms
        .iter()
        .flat_map(|&atom_idx| &orders.order_sensitive[atom_idx])
        .filter_map(|module_idx| {
          let position = view.entry.position.get(module_idx)?;
          Some((orders.module_table[*module_idx].exec_order(), *position))
        })
        .collect()
    };
    let staying = placed(staying);
    placed(view.moved).iter().any(|&(moved_exec_order, moved_position)| {
      staying.iter().any(|&(exec_order, position)| {
        (moved_exec_order < exec_order) != (moved_position < position)
      })
    })
  }

  /// Whether the point at which the entry runs the node moves past a module of another chunk. The
  /// entry runs the node where it first needs one of its modules. When the moved atoms come first,
  /// the modules that were there before now run at the position of the moved atoms; when they come
  /// later, the moved atoms now run at the position of the others. A module that matters between
  /// those two positions changes sides, unless the node imports it.
  fn evaluation_point_crosses(
    &self,
    summaries: &mut NodeOrderSummaries,
    orders: &AtomExecutionOrders<'_>,
    view: &MoveInEntry<'_>,
  ) -> bool {
    let mut moved_span = Span::default();
    for &atom_idx in view.moved {
      moved_span.add_atom(orders, view.entry, atom_idx);
    }
    let staying_span =
      summaries.span(self, orders, view.node, view.entry_bit, view.entry, view.moved);
    let (Some(moved_first), Some(staying_first)) = (moved_span.first, staying_span.first) else {
      return false;
    };
    let (from, until) = if moved_first < staying_first {
      (moved_first, staying_span.last_order_sensitive)
    } else {
      (staying_first, moved_span.last_order_sensitive)
    };
    let Some(until) = until else {
      return false;
    };
    let owner = |module_idx: ModuleIdx| {
      orders.module_to_atom_idx[module_idx].map(|atom_idx| (atom_idx, self.node_by_atom[atom_idx]))
    };
    let is_imported = |atom_idx: &usize| {
      let owner = self.node_by_atom[*atom_idx];
      owner == view.node || view.imported.contains(owner)
    };
    let runs_before_node = |module_idx: ModuleIdx| match orders.module_to_atom_idx[module_idx] {
      Some(atom_idx) => is_imported(&atom_idx),
      None => self
        .external_importers
        .get(&module_idx)
        .is_some_and(|importers| importers.iter().any(is_imported)),
    };
    // The modules of the atoms that were in the node run before it. `owned_runs` remembers a range
    // of them in the order of the entry, so that a later move does not walk them again.
    let stays_in_node = |module_idx: ModuleIdx| {
      owner(module_idx)
        .is_some_and(|(atom_idx, node)| node == view.node && !view.moved.contains(&atom_idx))
    };
    let order_sensitive = &view.entry.order_sensitive;
    let runs = summaries.owned_runs.entry(view.node).or_default();
    let mut run = runs.get(&view.entry_bit).copied();
    let mut index = order_sensitive.partition_point(|&(position, _)| position <= from);
    let mut crosses = false;
    while let Some(&(position, module_idx)) = order_sensitive.get(index) {
      if position >= until {
        break;
      }
      if let Some((first, last)) = run
        && (first..=last).contains(&index)
      {
        index = last + 1;
        continue;
      }
      if stays_in_node(module_idx) {
        run = Some(match run {
          Some((first, last)) if index == last + 1 => (first, index),
          Some((first, last)) if index + 1 == first => (index, last),
          Some(run) if run.1 - run.0 > 0 => run,
          _ => (index, index),
        });
      } else if !runs_before_node(module_idx) {
        crosses = true;
        break;
      }
      index += 1;
    }
    if let Some(run) = run {
      runs.insert(view.entry_bit, run);
    }
    crosses
  }

  fn add_node(&mut self, bits: &BitSet) -> usize {
    let node = self.atoms_by_node.len();
    self.node_by_bits.insert(bits.clone(), node);
    self.bits_by_node.push(bits.clone());
    let facade_entry_bit =
      if bits.bit_count() == 1 { bits.index_of_one().next().map(|bit| bit as usize) } else { None };
    if let Some(entry_bit) = facade_entry_bit {
      self.facade_node_by_entry_bit[entry_bit] = Some(node);
    }
    self.atoms_by_node.push(vec![]);
    self.externals_by_node.push(FxHashMap::default());
    self.facade_entry_bit_by_node.push(facade_entry_bit);
    self.edges.add_node();
    self.marks.push(0);
    node
  }

  /// The facade edge of an entry. An entry whose module lands in a shared chunk keeps its own entry
  /// chunk as a facade. `compute_chunk_imports` makes that facade import the host chunk, to run
  /// the module and to re-export its interface. No module in the facade imports the entry module,
  /// so the atom edges cannot express this edge. Without this edge, a fold of an atom into a facade
  /// whose host statically depends on that atom yields a chunk cycle. That cycle evaluates in the
  /// wrong order (rolldown#10734). The cycle graph over-approximates, so a redundant facade edge
  /// costs only a missed merge.
  fn facade_edge(&self, entry_bit: usize) -> Option<(usize, usize)> {
    // If the facade node has no atoms, the entry chunk holds nothing. A facade without modules
    // declares no symbol, so nothing can depend on it, and the omitted node cannot lie on a cycle.
    let facade = self.facade_node_by_entry_bit[entry_bit]?;
    if self.atoms_by_node[facade].is_empty() {
      return None;
    }
    // A `None` host means that the entry module is external or excluded. In general, to skip an
    // edge is not safe, because the cycle graph may never miss an edge. Here the skip drops no real
    // edge: `split_chunks` assigns such a module to no chunk, so the facade has no host to import.
    // An included normal entry module always has an atom. `determine_reachable_modules_for_entry`
    // sets the entry's own bit on its module, and `split_chunks` asserts that the bits of an
    // included module are not empty.
    let host = self.node_by_atom[self.host_atom_by_entry_bit[entry_bit]?];
    (facade != host).then_some((facade, host))
  }

  /// Moves the atom and its edges from node `from` to node `to`.
  fn move_atom(&mut self, atom_idx: usize, from: usize, to: usize) {
    // Three kinds of facade edge can change: the edges of the entries that the atom hosts, the edge
    // of the entry whose chunk is `from` if `from` loses its last atom, and the edge of the entry
    // whose chunk is `to` if `to` gets its first atom.
    let mut entry_bits = self.hosted_entry_bits_by_atom[atom_idx].clone();
    if self.atoms_by_node[from].len() == 1 {
      entry_bits.extend(self.facade_entry_bit_by_node[from]);
    }
    if self.atoms_by_node[to].is_empty() {
      entry_bits.extend(self.facade_entry_bit_by_node[to]);
    }
    entry_bits.sort_unstable();
    entry_bits.dedup();

    for &entry_bit in &entry_bits {
      if let Some((facade, host)) = self.facade_edge(entry_bit) {
        self.edges.remove(facade, host);
      }
    }
    for &dependency in &self.atom_dependencies[atom_idx] {
      let node = self.node_by_atom[dependency];
      if node != from {
        self.edges.remove(from, node);
      }
    }
    for &dependent in &self.atom_dependents[atom_idx] {
      let node = self.node_by_atom[dependent];
      if node != from {
        self.edges.remove(node, from);
      }
    }

    let position = self.atoms_by_node[from]
      .iter()
      .position(|&member| member == atom_idx)
      .expect("an atom is a member of its node");
    self.atoms_by_node[from].swap_remove(position);
    self.atoms_by_node[to].push(atom_idx);
    self.node_by_atom[atom_idx] = to;
    if let Some(orders) = &self.execution_order {
      for external in &orders.externals[atom_idx] {
        let from_externals = &mut self.externals_by_node[from];
        let count = from_externals.get_mut(external).expect("the node imports the external");
        *count -= 1;
        if *count == 0 {
          from_externals.remove(external);
        }
        *self.externals_by_node[to].entry(*external).or_default() += 1;
      }
    }

    for &dependency in &self.atom_dependencies[atom_idx] {
      let node = self.node_by_atom[dependency];
      if node != to {
        self.edges.add(to, node);
      }
    }
    for &dependent in &self.atom_dependents[atom_idx] {
      let node = self.node_by_atom[dependent];
      if node != to {
        self.edges.add(node, to);
      }
    }
    for &entry_bit in &entry_bits {
      if let Some((facade, host)) = self.facade_edge(entry_bit) {
        self.edges.add(facade, host);
      }
    }
  }

  /// Whether a path leads from `node`, which received the moved atom, back to itself. This decides
  /// the move when the graph was acyclic before the move, because every edge that the move added
  /// touches `node`:
  ///
  /// - The edges of the atom now start or end at `node`.
  /// - The facade edges of the entries that the atom hosts now end at `node`, the new host.
  /// - If `node` got its first atom, the facade edge of its entry starts at `node`.
  /// - If `from` lost its last atom, the move removed the facade edge of its entry. It added none.
  ///
  /// A new cycle must use an added edge, so it passes through `node`.
  fn reaches_itself(&mut self, node: usize) -> bool {
    if self.mark == u32::MAX {
      self.marks.fill(0);
      self.mark = 0;
    }
    self.mark += 1;
    let mut stack: Vec<usize> = self.edges.dependencies[node].keys().copied().collect();
    while let Some(next) = stack.pop() {
      if next == node {
        return true;
      }
      if self.marks[next] == self.mark {
        continue;
      }
      self.marks[next] = self.mark;
      stack.extend(self.edges.dependencies[next].keys().copied());
    }
    false
  }

  /// Whether the graph of the nodes with atoms has a cycle.
  fn has_cycle(&self) -> bool {
    const NEW: u8 = 0;
    const ON_PATH: u8 = 1;
    const DONE: u8 = 2;
    let node_count = self.atoms_by_node.len();
    let mut state = vec![NEW; node_count];
    let mut stack = vec![];
    for root in 0..node_count {
      if self.atoms_by_node[root].is_empty() || state[root] != NEW {
        continue;
      }
      stack.push((root, false));
      while let Some((node, exiting)) = stack.pop() {
        if exiting {
          state[node] = DONE;
          continue;
        }
        if state[node] != NEW {
          continue;
        }
        state[node] = ON_PATH;
        stack.push((node, true));
        for &dependency in self.edges.dependencies[node].keys() {
          match state[dependency] {
            NEW => stack.push((dependency, false)),
            ON_PATH => return true,
            _ => {}
          }
        }
      }
    }
    false
  }

  /// See `cyclic_component`. This is Kosaraju's algorithm: a post-order over the dependencies,
  /// then a search over the dependents from each node in reverse post-order. Each such search
  /// collects one component.
  fn only_cyclic_component(&self) -> FxHashSet<usize> {
    let node_count = self.atoms_by_node.len();
    let mut visited = vec![false; node_count];
    let mut post_order = Vec::with_capacity(node_count);
    let mut stack = vec![];
    for root in 0..node_count {
      if self.atoms_by_node[root].is_empty() || visited[root] {
        continue;
      }
      stack.push((root, false));
      while let Some((node, exiting)) = stack.pop() {
        if exiting {
          post_order.push(node);
          continue;
        }
        if visited[node] {
          continue;
        }
        visited[node] = true;
        stack.push((node, true));
        for &dependency in self.edges.dependencies[node].keys() {
          if !visited[dependency] {
            stack.push((dependency, false));
          }
        }
      }
    }

    let mut collected = vec![false; node_count];
    let mut cyclic_component = None;
    for &root in post_order.iter().rev() {
      if collected[root] {
        continue;
      }
      collected[root] = true;
      let mut component = vec![root];
      let mut idx = 0;
      while let Some(&node) = component.get(idx) {
        idx += 1;
        for &dependent in &self.edges.dependents[node] {
          if !collected[dependent] {
            collected[dependent] = true;
            component.push(dependent);
          }
        }
      }
      if component.len() > 1 {
        if cyclic_component.is_some() {
          return FxHashSet::default();
        }
        cyclic_component = Some(component);
      }
    }
    cyclic_component.into_iter().flatten().collect()
  }
}

#[derive(Default)]
struct NodeEdges {
  /// Per node, the dependency nodes and the number of atom edges and facade edges behind each one.
  /// The counts let a rejected move take its edges back.
  dependencies: Vec<FxHashMap<usize, u32>>,
  dependents: Vec<FxHashSet<usize>>,
}

impl NodeEdges {
  fn add_node(&mut self) {
    self.dependencies.push(FxHashMap::default());
    self.dependents.push(FxHashSet::default());
  }

  fn add(&mut self, from: usize, to: usize) {
    let count = self.dependencies[from].entry(to).or_insert(0);
    *count += 1;
    if *count == 1 {
      self.dependents[to].insert(from);
    }
  }

  fn remove(&mut self, from: usize, to: usize) {
    let count = self.dependencies[from].get_mut(&to).expect("the edge must exist");
    *count -= 1;
    if *count == 0 {
      self.dependencies[from].remove(&to);
      self.dependents[to].remove(&from);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  impl ReducedChunkGraph<'_> {
    /// Recomputes the edges from the atoms and checks the cycle state against them.
    fn assert_consistent(&self) {
      let node_count = self.atoms_by_node.len();
      let mut atoms_by_node = vec![vec![]; node_count];
      for (atom_idx, &node) in self.node_by_atom.iter().enumerate() {
        atoms_by_node[node].push(atom_idx);
      }
      let mut actual = self.atoms_by_node.clone();
      actual.iter_mut().for_each(|atoms| atoms.sort_unstable());
      assert_eq!(actual, atoms_by_node);

      let mut dependencies = vec![FxHashMap::<usize, u32>::default(); node_count];
      for (atom_idx, atom_dependencies) in self.atom_dependencies.iter().enumerate() {
        for &dependency in atom_dependencies {
          let (from, to) = (self.node_by_atom[atom_idx], self.node_by_atom[dependency]);
          if from != to {
            *dependencies[from].entry(to).or_default() += 1;
          }
        }
      }
      for entry_bit in 0..self.host_atom_by_entry_bit.len() {
        if let Some((facade, host)) = self.facade_edge(entry_bit) {
          *dependencies[facade].entry(host).or_default() += 1;
        }
      }
      assert_eq!(self.edges.dependencies, dependencies);
      for (from, node_dependencies) in dependencies.iter().enumerate() {
        for to in node_dependencies.keys() {
          assert!(self.edges.dependents[*to].contains(&from));
        }
      }
      let dependent_count: usize = self.edges.dependents.iter().map(FxHashSet::len).sum();
      assert_eq!(dependent_count, dependencies.iter().map(FxHashMap::len).sum::<usize>());

      assert_eq!(self.is_acyclic, !self.has_cycle());
      if !self.is_acyclic {
        assert_eq!(self.cyclic_component, self.only_cyclic_component());
      }
    }
  }

  /// The old global check. It rebuilds the chunk graph from the bits of the atoms and searches it.
  fn has_cycle(
    atom_bits: &[BitSet],
    atom_dependencies: &[Vec<usize>],
    host_atom_by_entry_bit: &[Option<usize>],
  ) -> bool {
    let mut chunk_by_bits = FxHashMap::default();
    let chunk_by_atom: Vec<usize> = atom_bits
      .iter()
      .map(|bits| {
        let next_chunk = chunk_by_bits.len();
        *chunk_by_bits.entry(bits.clone()).or_insert(next_chunk)
      })
      .collect();
    let mut facade_chunk_by_entry_bit = vec![None; host_atom_by_entry_bit.len()];
    for (atom_idx, bits) in atom_bits.iter().enumerate() {
      if bits.bit_count() == 1 {
        let entry_bit = bits.index_of_one().next().unwrap() as usize;
        facade_chunk_by_entry_bit[entry_bit] = Some(chunk_by_atom[atom_idx]);
      }
    }
    let mut dependencies = vec![FxHashSet::default(); chunk_by_bits.len()];
    for (atom_idx, atom_dependencies) in atom_dependencies.iter().enumerate() {
      for &dependency in atom_dependencies {
        let (from, to) = (chunk_by_atom[atom_idx], chunk_by_atom[dependency]);
        if from != to {
          dependencies[from].insert(to);
        }
      }
    }
    for (entry_bit, host_atom_idx) in host_atom_by_entry_bit.iter().enumerate() {
      if let (Some(host_atom_idx), Some(facade)) =
        (host_atom_idx, facade_chunk_by_entry_bit[entry_bit])
      {
        let host = chunk_by_atom[*host_atom_idx];
        if facade != host {
          dependencies[facade].insert(host);
        }
      }
    }
    // Kahn's algorithm keeps the oracle independent of the two searches and the component shortcut
    // in the module. The oracle rebuilds every edge, with the facade edges, without the module's
    // helpers.
    let mut in_degree = vec![0; dependencies.len()];
    for targets in &dependencies {
      for &target in targets {
        in_degree[target] += 1;
      }
    }
    let mut ready: Vec<_> = (0..dependencies.len()).filter(|&node| in_degree[node] == 0).collect();
    let mut removed = 0;
    while let Some(node) = ready.pop() {
      removed += 1;
      for &target in &dependencies[node] {
        in_degree[target] -= 1;
        if in_degree[target] == 0 {
          ready.push(target);
        }
      }
    }
    removed != dependencies.len()
  }

  fn bits(entry_count: u32, entry_bits: &[u32]) -> BitSet {
    let mut bits = BitSet::new(entry_count);
    for &entry_bit in entry_bits {
      bits.set_bit(entry_bit);
    }
    bits
  }

  /// Moves each `(atom, reduced bits)` in turn, checking the result against `has_cycle`.
  fn check_moves(
    entry_count: u32,
    atom_bits: &[&[u32]],
    atom_dependencies: &[Vec<usize>],
    host_atom_by_entry_bit: &[Option<usize>],
    moves: &[(usize, &[u32], bool)],
  ) {
    let mut current: Vec<BitSet> = atom_bits.iter().map(|set| bits(entry_count, set)).collect();
    let mut graph = ReducedChunkGraph::new(&current, atom_dependencies, host_atom_by_entry_bit);
    graph.assert_consistent();
    for &(atom_idx, reduced, expected) in moves {
      let reduced = bits(entry_count, reduced);
      let mut next = current.clone();
      next[atom_idx] = reduced.clone();
      assert_eq!(!has_cycle(&next, atom_dependencies, host_atom_by_entry_bit), expected);
      assert_eq!(graph.try_move(atom_idx, &reduced), expected);
      graph.assert_consistent();
      if expected {
        current = next;
      }
    }
  }

  #[test]
  fn a_fold_into_a_chunk_that_the_atom_reaches_is_rejected() {
    // Entry 0 loads atom 1, and entry 1 (atom 2) also loads it. A fold of atom 1 into entry 0 is
    // safe. It is not safe when atom 1 also depends on the chunk of entry 1.
    check_moves(
      2,
      &[&[0], &[0, 1], &[1]],
      &[vec![1], vec![], vec![1]],
      &[None, None],
      &[(1, &[0], true)],
    );
    check_moves(
      2,
      &[&[0], &[0, 1], &[1]],
      &[vec![1], vec![2], vec![1]],
      &[None, None],
      &[(1, &[0], false)],
    );
  }

  #[test]
  fn a_fold_into_a_facade_whose_host_reaches_the_atom_is_rejected() {
    // The module of entry 1 is in atom 0, a chunk that entry 0 shares. Atom 0 depends on atom 1.
    // So a fold of atom 1 into the chunk of entry 1 makes that facade import its host, and the
    // host imports the facade.
    check_moves(
      3,
      &[&[0, 1], &[1, 2]],
      &[vec![1], vec![]],
      &[None, Some(0), None],
      &[(1, &[1], false)],
    );
    check_moves(
      3,
      &[&[0, 1], &[1, 2]],
      &[vec![], vec![]],
      &[None, Some(0), None],
      &[(1, &[1], true)],
    );
  }

  #[test]
  fn a_move_that_breaks_the_only_cycle_is_accepted() {
    // Atoms 1 and 2 depend on each other, so the graph starts with a cycle. The check rejects a
    // move that keeps the cycle. A merge of the two atoms removes the cycle, and later moves get
    // the local check again.
    check_moves(
      3,
      &[&[0], &[0, 1], &[1], &[1, 2]],
      &[vec![1], vec![2], vec![1], vec![]],
      &[None, None, None],
      &[(3, &[2], false), (1, &[1], true), (3, &[1], true)],
    );
    // With two cycles, a move that breaks one of them keeps the other one.
    check_moves(
      3,
      &[&[0, 1], &[1], &[0, 2], &[2]],
      &[vec![1], vec![0], vec![3], vec![2]],
      &[None, None, None],
      &[(0, &[1], false), (2, &[2], false)],
    );
  }

  #[test]
  fn accepted_moves_update_the_edges_that_later_moves_see() {
    // Three accepted moves in a row. Atom 0 joins the chunk of atom 3, and that chunk gains the
    // chunk of atom 1 as a dependent. Atom 2 moves to a new chunk of its own. Atom 3 then leaves
    // the shared chunk for a new chunk, and atom 0 stays behind. In the second case, atom 3 depends
    // on atom 4, whose chunk reaches the chunk of atom 1, so the first move closes a cycle through
    // the shared chunk.
    check_moves(
      4,
      &[&[0, 1], &[1], &[1, 2], &[0]],
      &[vec![], vec![0], vec![1], vec![]],
      &[None, None, None, None],
      &[(0, &[0], true), (2, &[2], true), (3, &[0, 3], true)],
    );
    check_moves(
      4,
      &[&[0, 1], &[1], &[1, 2], &[0], &[0, 2, 3]],
      &[vec![], vec![0], vec![1], vec![4], vec![2]],
      &[None, None, None, None],
      &[(0, &[0], false)],
    );
  }

  #[test]
  fn atoms_that_cannot_move_one_by_one_move_together() {
    let check = |atom_bits: &[&[u32]],
                 atom_dependencies: &[Vec<usize>],
                 rejected: &[usize],
                 group: &[usize]| {
      let current: Vec<BitSet> = atom_bits.iter().map(|set| bits(4, set)).collect();
      let target = bits(4, &[0, 1]);
      let mut next = current.clone();
      group.iter().for_each(|&atom_idx| next[atom_idx] = target.clone());
      assert!(!has_cycle(&next, atom_dependencies, &[None; 4]));
      let mut graph = ReducedChunkGraph::new(&current, atom_dependencies, &[None; 4]);
      for &atom_idx in rejected {
        assert!(!graph.try_move(atom_idx, &target));
        graph.assert_consistent();
      }
      assert!(graph.try_move_atoms(group, &target));
      graph.assert_consistent();
    };
    // Atom 0 depends on atom 1, and atom 1 depends on atom 2. Atom 2 cannot join the chunk of
    // atom 0 before atom 1 does.
    check(&[&[0, 1], &[0, 1, 2], &[0, 1, 2, 3]], &[vec![1], vec![2], vec![]], &[2], &[2, 1]);
    // Atoms 1 and 2 depend on each other, so the graph starts with a cycle. Neither atom can leave
    // it alone.
    check(&[&[0, 1], &[0, 1, 2], &[0, 1, 3]], &[vec![1], vec![2], vec![1]], &[1, 2], &[1, 2]);
  }

  struct Rng(u64);

  impl Rng {
    fn below(&mut self, bound: usize) -> usize {
      self.0 ^= self.0 << 13;
      self.0 ^= self.0 >> 7;
      self.0 ^= self.0 << 17;
      usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }
  }

  fn check_move(
    graph: &mut ReducedChunkGraph<'_>,
    current: &mut [BitSet],
    atom: usize,
    to: &BitSet,
  ) {
    if current[atom] == *to {
      return;
    }
    let previous = std::mem::replace(&mut current[atom], to.clone());
    let expected = !has_cycle(current, graph.atom_dependencies, graph.host_atom_by_entry_bit);
    assert_eq!(graph.try_move(atom, to), expected, "atom {atom}, from {previous:?}, to {to:?}");
    if !expected {
      current[atom] = previous;
    }
    graph.assert_consistent();
    for (atom, bits) in current.iter().enumerate() {
      assert_eq!(graph.node_by_atom[atom], graph.node_by_bits[bits]);
    }
  }

  #[test]
  fn exhaustive_small_graphs_match_a_full_cycle_check() {
    // All graphs on the three non-empty bitsets of two entries, all host assignments (with absent
    // hosts), and all pairs of moves. The second move checks the state after an accepted move and
    // after a rollback. It also reuses a node that the first move emptied.
    let initial = vec![bits(2, &[0]), bits(2, &[1]), bits(2, &[0, 1])];
    let edges = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    for edge_mask in 0..1 << edges.len() {
      let mut dependencies = vec![vec![]; 3];
      for (bit, &(from, to)) in edges.iter().enumerate() {
        if edge_mask & (1 << bit) != 0 {
          dependencies[from].push(to);
        }
      }
      for host0 in 0..4 {
        for host1 in 0..4 {
          let hosts = [host0, host1].map(|host| (host < 3).then_some(host));
          for first_atom in 0..3 {
            for first_bits in &initial {
              for second_atom in 0..3 {
                for second_bits in &initial {
                  let mut current = initial.clone();
                  let mut graph = ReducedChunkGraph::new(&current, &dependencies, &hosts);
                  check_move(&mut graph, &mut current, first_atom, first_bits);
                  check_move(&mut graph, &mut current, second_atom, second_bits);
                }
              }
            }
          }
        }
      }
    }
  }

  #[test]
  fn long_move_sequences_with_wide_bitsets_match_a_full_cycle_check() {
    // Bitsets wider than a byte and wider than a word. The random test uses at most five entry bits
    // and sixteen moves, so it seldom creates and reuses empty nodes, and it seldom puts many atom
    // edges behind one node edge.
    for seed in 1..=128 {
      let mut rng = Rng(seed * 0x9E37_79B9);
      let entry_count = [9, 65, 129][rng.below(3)];
      let atom_count = 8 + rng.below(25);
      let mut current = Vec::new();
      while current.len() < atom_count {
        let mut set = BitSet::new(entry_count);
        for _ in 0..=rng.below(12) {
          set.set_bit(u32::try_from(rng.below(entry_count as usize)).unwrap());
        }
        if !current.contains(&set) {
          current.push(set);
        }
      }
      let dependencies = (0..atom_count)
        .map(|atom| {
          (0..atom_count)
            .flat_map(|dep| {
              let count = if dep != atom && (seed % 2 == 0 || dep < atom) && rng.below(8) == 0 {
                // Parallel atom edges test the edge counts without facade edges.
                1 + rng.below(3)
              } else {
                0
              };
              std::iter::repeat_n(dep, count)
            })
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
      let hosts = (0..entry_count)
        .map(|_| (rng.below(3) != 0).then(|| rng.below(atom_count)))
        .collect::<Vec<_>>();
      let mut graph = ReducedChunkGraph::new(&current, &dependencies, &hosts);
      for step in 0..256 {
        let atom = rng.below(atom_count);
        let to = match step % 3 {
          0 => current[rng.below(atom_count)].clone(),
          1 => {
            let mut reduced = current[atom].clone();
            for bit in current[atom].index_of_one() {
              if reduced.bit_count() > 1 && rng.below(2) == 0 {
                reduced.clear_bit(bit);
              }
            }
            reduced
          }
          _ => bits(entry_count, &[u32::try_from(rng.below(entry_count as usize)).unwrap()]),
        };
        // Test the wraparound of the search mark without billions of searches before it.
        if step == 128 {
          graph.mark = u32::MAX;
        }
        check_move(&mut graph, &mut current, atom, &to);
      }
    }
  }

  #[test]
  fn random_moves_match_a_full_cycle_check() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let cases = std::env::var("ROLLDOWN_CYCLE_FUZZ_CASES")
      .map_or(4000, |value| value.parse::<usize>().expect("a positive case count"));
    for case in 0..cases {
      let entry_count = 1 + rng.below(5);
      let all_bits = (1..1_u32 << entry_count)
        .map(|mask| {
          let set = (0..u32::try_from(entry_count).unwrap())
            .filter(|bit| mask & (1 << bit) != 0)
            .collect::<Vec<_>>();
          bits(u32::try_from(entry_count).unwrap(), &set)
        })
        .collect::<Vec<_>>();
      // Atoms start with different bits, as `group_modules_by_dependent_entries` makes them.
      let mut atom_bits = all_bits.clone();
      for idx in (1..atom_bits.len()).rev() {
        atom_bits.swap(idx, rng.below(idx + 1));
      }
      atom_bits.truncate(1 + rng.below(atom_bits.len().min(10)));
      let atom_count = atom_bits.len();
      let edge_chance = 1 + rng.below(4);
      // Half of the graphs start acyclic, so that their moves get the local check from the start.
      let mut rank = (0..atom_count).collect::<Vec<_>>();
      for idx in (1..atom_count).rev() {
        rank.swap(idx, rng.below(idx + 1));
      }
      let acyclic = rng.below(2) == 0;
      let atom_dependencies = (0..atom_count)
        .map(|atom_idx| {
          (0..atom_count)
            .filter(|&dependency| {
              dependency != atom_idx
                && (!acyclic || rank[atom_idx] < rank[dependency])
                && rng.below(8) < edge_chance
            })
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
      let host_atom_by_entry_bit = (0..entry_count)
        .map(|_| (rng.below(4) != 0).then(|| rng.below(atom_count)))
        .collect::<Vec<_>>();

      let mut graph =
        ReducedChunkGraph::new(&atom_bits, &atom_dependencies, &host_atom_by_entry_bit);
      graph.assert_consistent();
      for step in 0..16 {
        // Odd steps move two atoms together.
        let mut moved_atoms = vec![rng.below(atom_count)];
        if step % 2 == 1 && atom_count > 1 {
          moved_atoms.push((moved_atoms[0] + 1 + rng.below(atom_count - 1)) % atom_count);
        }
        let reduced = &all_bits[rng.below(all_bits.len())];
        if moved_atoms.iter().any(|&atom_idx| *reduced == atom_bits[atom_idx]) {
          continue;
        }
        let mut next = atom_bits.clone();
        moved_atoms.iter().for_each(|&atom_idx| next[atom_idx] = reduced.clone());
        let expected = !has_cycle(&next, &atom_dependencies, &host_atom_by_entry_bit);
        assert_eq!(
          graph.try_move_atoms(&moved_atoms, reduced),
          expected,
          "case {case}, step {step}"
        );
        graph.assert_consistent();
        if expected {
          atom_bits = next;
        }
      }
    }
  }
}
