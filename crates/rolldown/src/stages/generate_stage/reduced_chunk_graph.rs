use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

/// The chunk graph that the current dependent-entry bits of the atoms produce.
/// `optimize_dynamic_entry_bits` moves atoms to reduced bits, and the graph follows each move.
/// Atoms with equal bits form one node (a chunk). A node depends on another node when one of its
/// atoms depends on an atom of the other node. A node also depends on another node through a
/// facade edge (see `facade_edge`).
///
/// Every edge that a move adds starts or ends at the node that receives the atom (see
/// `reaches_itself`), so every cycle a move closes runs through that node, and the check is one
/// search from it. The cycle graph over-approximates, so it can hold a cycle before any move; a
/// cycle elsewhere predates the move and does not reject it.
/// See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
pub(super) struct ReducedChunkGraph<'a> {
  atom_dependencies: &'a [Vec<usize>],
  atom_dependents: Vec<Vec<usize>>,
  /// The under-approximated atom edges: imports the emitted chunks are certain to have. Empty
  /// unless `check_execution_order` turned the execution-order check on.
  certain_atom_dependencies: &'a [Vec<usize>],
  /// Per atom, the execution-order range of its modules with a side effect of their own.
  atom_side_effect_exec_orders: &'a [ExecOrderRange],
  host_atom_by_entry_bit: &'a [Option<usize>],
  hosted_entry_bits_by_atom: Vec<SmallVec<[usize; 1]>>,
  node_by_bits: FxHashMap<BitSet, usize>,
  node_by_atom: Vec<usize>,
  /// A node without atoms is not a chunk. It has no edges.
  atoms_by_node: Vec<Vec<usize>>,
  /// The entry bit of a node whose bits hold only that bit. Such a node is that entry's chunk.
  facade_entry_bit_by_node: Vec<Option<usize>>,
  facade_node_by_entry_bit: Vec<Option<usize>>,
  edges: NodeEdges,
  /// Per node, the `mark` of the last search that visited it.
  marks: Vec<u32>,
  mark: u32,
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
      certain_atom_dependencies: &[],
      atom_side_effect_exec_orders: &[],
      host_atom_by_entry_bit,
      hosted_entry_bits_by_atom: vec![SmallVec::new(); atom_count],
      node_by_bits: FxHashMap::default(),
      node_by_atom: Vec::with_capacity(atom_count),
      atoms_by_node: Vec::with_capacity(atom_count),
      facade_entry_bit_by_node: Vec::with_capacity(atom_count),
      facade_node_by_entry_bit: vec![None; host_atom_by_entry_bit.len()],
      edges: NodeEdges::default(),
      marks: Vec::with_capacity(atom_count),
      mark: 0,
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
    graph
  }

  /// Also rejects a move that runs one of the atom's side effects after a side effect that used to
  /// run later (see `runs_side_effects_late`).
  pub(super) fn check_execution_order(
    &mut self,
    certain_atom_dependencies: &'a [Vec<usize>],
    atom_side_effect_exec_orders: &'a [ExecOrderRange],
  ) {
    self.certain_atom_dependencies = certain_atom_dependencies;
    self.atom_side_effect_exec_orders = atom_side_effect_exec_orders;
  }

  /// Moves the atom to the node of `bits`, or keeps it where it was and says why not.
  pub(super) fn try_move(&mut self, atom_idx: usize, bits: &BitSet) -> Result<(), MoveVeto> {
    let from = self.node_by_atom[atom_idx];
    let to = match self.node_by_bits.get(bits) {
      Some(&node) => node,
      None => self.add_node(bits),
    };
    debug_assert_ne!(from, to, "the reduced bits must differ from the current bits");
    self.move_atom(atom_idx, from, to);
    let veto = if self.reaches_itself(to) {
      Some(MoveVeto::Cycle)
    } else if self.runs_side_effects_late(atom_idx, to) {
      Some(MoveVeto::SideEffectOrder)
    } else {
      None
    };
    match veto {
      Some(veto) => {
        self.move_atom(atom_idx, to, from);
        Err(veto)
      }
      None => Ok(()),
    }
  }

  fn add_node(&mut self, bits: &BitSet) -> usize {
    let node = self.atoms_by_node.len();
    self.node_by_bits.insert(bits.clone(), node);
    let facade_entry_bit =
      if bits.bit_count() == 1 { bits.index_of_one().next().map(|bit| bit as usize) } else { None };
    if let Some(entry_bit) = facade_entry_bit {
      self.facade_node_by_entry_bit[entry_bit] = Some(node);
    }
    self.atoms_by_node.push(vec![]);
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

  /// Whether a path leads from `node`, which received the moved atom, back to itself. Every edge
  /// that the move added touches `node`:
  ///
  /// - The edges of the atom now start or end at `node`.
  /// - The facade edges of the entries that the atom hosts now end at `node`, the new host.
  /// - If `node` got its first atom, the facade edge of its entry starts at `node`.
  /// - If `from` lost its last atom, the move removed the facade edge of its entry. It added none.
  ///
  /// A new cycle must use an added edge, so it passes through `node`.
  fn reaches_itself(&mut self, node: usize) -> bool {
    self.next_mark();
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

  /// Whether the move of `atom_idx` into `node` runs one of the atom's side effects late. The
  /// atom's modules now run in the body of `node`, after every chunk `node` imports. A chunk in
  /// that closure with a side effect later than the atom's earliest one ran after the atom before
  /// the move, unless the atom was certain to import it. The closure reads the over-approximated
  /// edges so that no real import is missed; the exemption reads the under-approximated ones so
  /// that an invented import never excuses a chunk.
  fn runs_side_effects_late(&mut self, atom_idx: usize, node: usize) -> bool {
    let Some(&Some((earliest_side_effect, _))) = self.atom_side_effect_exec_orders.get(atom_idx)
    else {
      return false;
    };
    let mut loaded_before = None;
    self.next_mark();
    let mut stack: Vec<usize> = self.edges.dependencies[node].keys().copied().collect();
    while let Some(next) = stack.pop() {
      if next == node || self.marks[next] == self.mark {
        continue;
      }
      self.marks[next] = self.mark;
      if self.latest_side_effect(next).is_some_and(|latest| latest > earliest_side_effect)
        && !loaded_before
          .get_or_insert_with(|| self.certainly_loaded_before(atom_idx, node))
          .contains(&next)
      {
        return true;
      }
      stack.extend(self.edges.dependencies[next].keys().copied());
    }
    false
  }

  /// The nodes, `node` excluded, that the atom's certain imports load before it.
  fn certainly_loaded_before(&self, atom_idx: usize, node: usize) -> FxHashSet<usize> {
    let certain_dependency_nodes =
      |atom: usize| self.certain_atom_dependencies[atom].iter().map(|&dep| self.node_by_atom[dep]);
    let mut loaded = FxHashSet::default();
    let mut stack: Vec<usize> = certain_dependency_nodes(atom_idx).collect();
    while let Some(next) = stack.pop() {
      if next != node && loaded.insert(next) {
        for &member in &self.atoms_by_node[next] {
          stack.extend(certain_dependency_nodes(member));
        }
      }
    }
    loaded
  }

  fn latest_side_effect(&self, node: usize) -> Option<u32> {
    self.atoms_by_node[node]
      .iter()
      .filter_map(|&atom| self.atom_side_effect_exec_orders[atom].map(|(_, latest)| latest))
      .max()
  }

  fn next_mark(&mut self) {
    if self.mark == u32::MAX {
      self.marks.fill(0);
      self.mark = 0;
    }
    self.mark += 1;
  }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MoveVeto {
  Cycle,
  /// See `runs_side_effects_late`. Another atom's move may remove the hazard, so the caller may
  /// try the move again.
  SideEffectOrder,
}

/// The execution-order range `(earliest, latest)` of some modules with a side effect of their own,
/// `None` when they have none.
pub(super) type ExecOrderRange = Option<(u32, u32)>;

pub(super) fn widen_exec_order_range(
  range: ExecOrderRange,
  other: ExecOrderRange,
) -> ExecOrderRange {
  match (range, other) {
    (Some((earliest, latest)), Some((other_earliest, other_latest))) => {
      Some((earliest.min(other_earliest), latest.max(other_latest)))
    }
    (range, None) => range,
    (None, other) => other,
  }
}

#[derive(Default)]
struct NodeEdges {
  /// Per node, the dependency nodes and the number of atom edges and facade edges behind each one.
  /// The counts let a rejected move take its edges back.
  dependencies: Vec<FxHashMap<usize, u32>>,
}

impl NodeEdges {
  fn add_node(&mut self) {
    self.dependencies.push(FxHashMap::default());
  }

  fn add(&mut self, from: usize, to: usize) {
    *self.dependencies[from].entry(to).or_insert(0) += 1;
  }

  fn remove(&mut self, from: usize, to: usize) {
    let count = self.dependencies[from].get_mut(&to).expect("the edge must exist");
    *count -= 1;
    if *count == 0 {
      self.dependencies[from].remove(&to);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  impl ReducedChunkGraph<'_> {
    /// Recomputes the edges from the atoms and checks them against the graph's.
    fn assert_consistent(&self) {
      let node_count = self.atoms_by_node.len();
      let mut atoms_by_node = vec![vec![]; node_count];
      for (atom_idx, &node) in self.node_by_atom.iter().enumerate() {
        atoms_by_node[node].push(atom_idx);
      }
      let mut sorted_atoms_by_node = self.atoms_by_node.clone();
      sorted_atoms_by_node.iter_mut().for_each(|atoms| atoms.sort_unstable());
      assert_eq!(sorted_atoms_by_node, atoms_by_node);

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
    }
  }

  /// Rebuilds the chunk graph from the bits of the atoms, without the module's helpers, and
  /// reports whether the chunk of `bits` lies on a cycle.
  fn chunk_on_cycle(
    atom_bits: &[BitSet],
    atom_dependencies: &[Vec<usize>],
    host_atom_by_entry_bit: &[Option<usize>],
    bits: &BitSet,
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
    let start = chunk_by_bits[bits];
    let mut reached = FxHashSet::default();
    let mut queue: Vec<usize> = dependencies[start].iter().copied().collect();
    while let Some(node) = queue.pop() {
      if reached.insert(node) {
        queue.extend(dependencies[node].iter().copied());
      }
    }
    reached.contains(&start)
  }

  fn bits(entry_count: u32, entry_bits: &[u32]) -> BitSet {
    let mut bits = BitSet::new(entry_count);
    for &entry_bit in entry_bits {
      bits.set_bit(entry_bit);
    }
    bits
  }

  /// Moves each `(atom, reduced bits)` in turn, checking the result against `chunk_on_cycle`.
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
      assert_eq!(
        !chunk_on_cycle(&next, atom_dependencies, host_atom_by_entry_bit, &reduced),
        expected
      );
      assert_eq!(graph.try_move(atom_idx, &reduced).is_ok(), expected);
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
  fn a_cycle_elsewhere_does_not_reject_a_move() {
    // Atoms 1 and 2 depend on each other, so the graph starts with a cycle. A move into a chunk
    // off that cycle is accepted, a move into a chunk on it is rejected, and a merge of the two
    // atoms removes the cycle.
    check_moves(
      3,
      &[&[0], &[0, 1], &[1], &[1, 2]],
      &[vec![1], vec![2], vec![1], vec![]],
      &[None, None, None],
      &[(3, &[2], true), (3, &[1], false), (1, &[1], true), (3, &[1], true)],
    );
    // With two cycles, a move that breaks one of them is accepted although the other one stays.
    check_moves(
      3,
      &[&[0, 1], &[1], &[0, 2], &[2]],
      &[vec![1], vec![0], vec![3], vec![2]],
      &[None, None, None],
      &[(0, &[1], true), (2, &[2], true)],
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
  fn a_fold_that_runs_a_side_effect_late_is_rejected() {
    // Entry 0 (atom 0) imports atoms 1 and 2, and atom 2 has the earlier side effect. A fold of
    // atom 2 into entry 0 runs it after atom 1, unless atom 2 is certain to import atom 1 itself.
    let atom_bits = [bits(3, &[0]), bits(3, &[0, 1]), bits(3, &[0, 2])];
    let side_effect_exec_orders = [None, Some((5, 5)), Some((3, 3))];
    let hosts = [None; 3];
    for (atom_2_dependencies, expected) in
      [(vec![], Err(MoveVeto::SideEffectOrder)), (vec![1], Ok(()))]
    {
      let dependencies = vec![vec![1, 2], vec![], atom_2_dependencies];
      let mut graph = ReducedChunkGraph::new(&atom_bits, &dependencies, &hosts);
      graph.check_execution_order(&dependencies, &side_effect_exec_orders);
      assert_eq!(graph.try_move(2, &bits(3, &[0])), expected);
      graph.assert_consistent();
    }
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
    let expected =
      !chunk_on_cycle(current, graph.atom_dependencies, graph.host_atom_by_entry_bit, to);
    assert_eq!(
      graph.try_move(atom, to).is_ok(),
      expected,
      "atom {atom}, from {previous:?}, to {to:?}"
    );
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
      // Half of the graphs start acyclic, the other half usually start with a cycle.
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
        let atom_idx = rng.below(atom_count);
        let reduced = &all_bits[rng.below(all_bits.len())];
        if *reduced == atom_bits[atom_idx] {
          continue;
        }
        let mut next = atom_bits.clone();
        next[atom_idx] = reduced.clone();
        let expected = !chunk_on_cycle(&next, &atom_dependencies, &host_atom_by_entry_bit, reduced);
        assert_eq!(graph.try_move(atom_idx, reduced).is_ok(), expected, "case {case}, step {step}");
        graph.assert_consistent();
        if expected {
          atom_bits = next;
        }
      }
    }
  }
}
