use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

/// The chunk graph that the atoms' current dependent-entry bits produce, kept up to date while
/// `optimize_dynamic_entry_bits` moves atoms to reduced bits. Atoms with equal bits form one node
/// (a chunk). A node depends on another node when one of its atoms depends on an atom of the
/// other node, and through the facade edges (see `facade_edge`).
///
/// Every edge that a move adds starts or ends at the node that receives the atom (see
/// `reaches_itself`). So while the graph is acyclic, a move closes a cycle exactly when a path
/// leads from that node back to itself, and the check is one search from that node. A move only
/// removes edges of the node that the atom leaves, so while the graph has a cycle, only a move out
/// of a node on every cycle can make the graph acyclic, and only such a move needs a full check.
/// See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
pub(super) struct ReducedChunkGraph<'a> {
  atom_dependencies: &'a [Vec<usize>],
  atom_dependents: Vec<Vec<usize>>,
  host_atom_by_entry_bit: &'a [Option<usize>],
  hosted_entry_bits_by_atom: Vec<SmallVec<[usize; 1]>>,
  node_by_bits: FxHashMap<BitSet, usize>,
  node_by_atom: Vec<usize>,
  /// A node without atoms is not a chunk: it has no edges.
  atom_count_by_node: Vec<usize>,
  /// The entry bit of a node whose bits hold only that entry, so the node is that entry's chunk.
  facade_entry_bit_by_node: Vec<Option<usize>>,
  facade_node_by_entry_bit: Vec<Option<usize>>,
  edges: NodeEdges,
  is_acyclic: bool,
  /// While the graph has a cycle: the nodes of its only strongly connected component with more
  /// than one node, or no nodes if it has more such components. Each cycle lies in one such
  /// component, and a move removes only edges of the node that the atom leaves. So a move out of a
  /// node outside this set keeps a cycle.
  cyclic_component: FxHashSet<usize>,
  /// Per node, the `mark` of the last search that visited it.
  marks: Vec<u32>,
  mark: u32,
}

impl<'a> ReducedChunkGraph<'a> {
  /// `atom_bits` are the initial dependent-entry bits of the atoms, which are all different.
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
      atom_count_by_node: Vec::with_capacity(atom_count),
      facade_entry_bit_by_node: Vec::with_capacity(atom_count),
      facade_node_by_entry_bit: vec![None; host_atom_by_entry_bit.len()],
      edges: NodeEdges::default(),
      is_acyclic: false,
      cyclic_component: FxHashSet::default(),
      marks: Vec::with_capacity(atom_count),
      mark: 0,
    };
    // Each atom starts as the only atom of its own node, so node and atom indices match.
    for bits in atom_bits {
      let node = graph.add_node(bits);
      debug_assert_eq!(node, graph.node_by_atom.len(), "atoms must have different bits");
      graph.node_by_atom.push(node);
      graph.atom_count_by_node[node] = 1;
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

  /// Moves the atom to the node of `bits` and returns true if the chunk graph stays acyclic.
  /// Otherwise, it keeps the atom where it was and returns false.
  pub(super) fn try_move(&mut self, atom_idx: usize, bits: &BitSet) -> bool {
    let from = self.node_by_atom[atom_idx];
    if !self.is_acyclic && !self.cyclic_component.contains(&from) {
      return false;
    }
    let to = match self.node_by_bits.get(bits) {
      Some(&node) => node,
      None => self.add_node(bits),
    };
    debug_assert_ne!(from, to, "the reduced bits must differ from the current bits");
    self.move_atom(atom_idx, from, to);
    // While the graph has a cycle, the move can remove every old cycle, but it can also close new
    // cycles through `to`, so only a full check is exact. A rejected move leaves
    // `cyclic_component` correct.
    let acyclic = if self.is_acyclic { !self.reaches_itself(to) } else { !self.has_cycle() };
    if acyclic {
      self.is_acyclic = true;
    } else {
      self.move_atom(atom_idx, to, from);
    }
    acyclic
  }

  fn add_node(&mut self, bits: &BitSet) -> usize {
    let node = self.atom_count_by_node.len();
    self.node_by_bits.insert(bits.clone(), node);
    let facade_entry_bit =
      if bits.bit_count() == 1 { bits.index_of_one().next().map(|bit| bit as usize) } else { None };
    if let Some(entry_bit) = facade_entry_bit {
      self.facade_node_by_entry_bit[entry_bit] = Some(node);
    }
    self.atom_count_by_node.push(0);
    self.facade_entry_bit_by_node.push(facade_entry_bit);
    self.edges.add_node();
    self.marks.push(0);
    node
  }

  /// The facade edge of an entry. An entry whose module lands in a shared chunk keeps its own entry
  /// chunk as a facade, and `compute_chunk_imports` makes that facade import the host chunk, to
  /// run the module and to re-export its interface. No module in the facade imports the entry
  /// module, so the atom edges cannot express this edge. Without it, folding an atom into a facade
  /// whose host statically depends on that atom yields a chunk cycle that evaluates in the wrong
  /// order (rolldown#10734). The cycle graph over-approximates, so a redundant facade edge only
  /// costs a missed merge.
  fn facade_edge(&self, entry_bit: usize) -> Option<(usize, usize)> {
    // No facade node with atoms means the entry chunk holds nothing: a module-less facade declares
    // no symbol, so nothing can depend back on it and the omitted node cannot sit on a cycle.
    let facade = self.facade_node_by_entry_bit[entry_bit]?;
    if self.atom_count_by_node[facade] == 0 {
      return None;
    }
    // A `None` host means the entry module is external or excluded. Skipping is not a safe
    // approximation in general — the cycle graph may never miss an edge — but it drops no real
    // edge: `split_chunks` assigns such a module to no chunk, so the facade has no host to import.
    // An included normal entry module always has an atom — `determine_reachable_modules_for_entry`
    // sets the entry's own bit on its module, and `split_chunks` asserts that an included module's
    // bits are non-empty.
    let host = self.node_by_atom[self.host_atom_by_entry_bit[entry_bit]?];
    (facade != host).then_some((facade, host))
  }

  /// Moves the atom and its edges from node `from` to node `to`.
  fn move_atom(&mut self, atom_idx: usize, from: usize, to: usize) {
    // The facade edges that can change: those of the entries that the atom hosts, of the entry
    // whose chunk `from` stops being, and of the entry whose chunk `to` starts being.
    let mut entry_bits = self.hosted_entry_bits_by_atom[atom_idx].clone();
    if self.atom_count_by_node[from] == 1 {
      entry_bits.extend(self.facade_entry_bit_by_node[from]);
    }
    if self.atom_count_by_node[to] == 0 {
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

    self.atom_count_by_node[from] -= 1;
    self.atom_count_by_node[to] += 1;
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

  /// Whether a path leads from `node`, which received the moved atom, back to itself. This decides
  /// the move when the graph was acyclic before it, because every edge the move added touches
  /// `node`: the atom's own edges now start or end at `node`; the facade edges of the entries the
  /// atom hosts now end at `node`, the new host; the facade edge of the entry whose chunk `node`
  /// starts being starts at `node`; and the entry whose chunk `from` stops being only loses its
  /// edge. A new cycle must use an added edge, so it passes through `node`.
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
    let node_count = self.atom_count_by_node.len();
    let mut state = vec![NEW; node_count];
    let mut stack = vec![];
    for root in 0..node_count {
      if self.atom_count_by_node[root] == 0 || state[root] != NEW {
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

  /// See `cyclic_component`. Kosaraju's algorithm: a post-order over the dependencies, then a
  /// search over the dependents from each node in reverse post-order collects one component.
  fn only_cyclic_component(&self) -> FxHashSet<usize> {
    let node_count = self.atom_count_by_node.len();
    let mut visited = vec![false; node_count];
    let mut post_order = Vec::with_capacity(node_count);
    let mut stack = vec![];
    for root in 0..node_count {
      if self.atom_count_by_node[root] == 0 || visited[root] {
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
  /// Per node: the dependency nodes and the number of atom and facade edges behind each one, so
  /// that a rejected move can take its edges back.
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
      let node_count = self.atom_count_by_node.len();
      let mut atom_count_by_node = vec![0; node_count];
      for &node in &self.node_by_atom {
        atom_count_by_node[node] += 1;
      }
      assert_eq!(self.atom_count_by_node, atom_count_by_node);

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

  /// The previous global check: rebuilds the chunk graph from the atoms' bits and searches it.
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
    // Kahn's algorithm keeps the oracle independent of both production DFS checks and the SCC
    // shortcut. Rebuild every edge, including facade edges, without using production helpers.
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
    // Entry 0 loads atom 1, which entry 1 (atom 2) also loads. Folding atom 1 into entry 0 is
    // safe until atom 1 also depends on entry 1's chunk.
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
    // Entry 1's module is in atom 0, a chunk shared with entry 0. Atom 0 depends on atom 1, so
    // folding atom 1 into entry 1's chunk makes that facade import its host, which imports it.
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
    // Atoms 1 and 2 depend on each other, so the graph starts with a cycle. A move that keeps the
    // cycle is rejected. Merging the two atoms removes the cycle, and later moves get the local
    // check again.
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
    // Three accepted moves in a row: atom 0 joins atom 3's chunk, which gains atom 1's chunk as a
    // dependent; atom 2 moves to a new chunk of its own; atom 3 then leaves the shared chunk for a
    // new chunk while atom 0 stays behind. With atom 3 depending on atom 4, whose chunk reaches
    // atom 1's chunk, the first move closes a cycle through the shared chunk instead.
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
    // All graphs on the three nonempty bitsets of two entries, all host assignments (including
    // absent hosts), and all pairs of moves. The second move checks both accepted state and
    // rollback, including reusing a node emptied by the first move.
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
    // Cross byte and word boundaries. The old fuzzer uses at most five entry bits and sixteen
    // moves, which rarely creates and reuses empty nodes or combines many edge contributions.
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
                // Parallel atom edges exercise reference counts independently of facade edges.
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
        // Exercise the search-generation wraparound without billions of preceding searches.
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
        let atom_idx = rng.below(atom_count);
        let reduced = &all_bits[rng.below(all_bits.len())];
        if *reduced == atom_bits[atom_idx] {
          continue;
        }
        let mut next = atom_bits.clone();
        next[atom_idx] = reduced.clone();
        let expected = !has_cycle(&next, &atom_dependencies, &host_atom_by_entry_bit);
        assert_eq!(graph.try_move(atom_idx, reduced), expected, "case {case}, step {step}");
        graph.assert_consistent();
        if expected {
          atom_bits = next;
        }
      }
    }
  }
}
