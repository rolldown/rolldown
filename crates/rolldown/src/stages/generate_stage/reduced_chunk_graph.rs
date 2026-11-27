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
/// `reaches_itself`). So while the graph is acyclic, a move closes a cycle exactly when a path
/// leads from that node back to itself. The check is one search from that node. A move removes
/// only edges of the node that the atom leaves. So while the graph has a cycle, only a move out of
/// a node that lies on every cycle can make the graph acyclic. Only such a move gets a full check.
/// A cycle through a manual group chunk is in the output with or without the fold. While every
/// cycle passes through such a chunk, a move is rejected only when one of the edges it adds lies
/// on a cycle: the move closes a new cycle. The check is one search per added edge.
/// See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
pub(super) struct ReducedChunkGraph<'a> {
  atom_dependencies: &'a [Vec<usize>],
  atom_dependents: Vec<Vec<usize>>,
  host_atom_by_entry_bit: &'a [Option<usize>],
  hosted_entry_bits_by_atom: Vec<SmallVec<[usize; 1]>>,
  node_by_bits: FxHashMap<BitSet, usize>,
  node_by_atom: Vec<usize>,
  /// A node without atoms is not a chunk. It has no edges.
  atom_count_by_node: Vec<usize>,
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
  /// Whether the graph started with a cycle and every cycle passes through a manual group chunk.
  /// No accepted move then closes a cycle, so every later cycle is one of those, and the flag stays.
  only_manual_group_cycles: bool,
  /// Per node, whether a manual group chunk.
  is_manual_group_node: Vec<bool>,
  /// Per node, the `mark` of the last search that visited it.
  marks: Vec<u32>,
  mark: u32,
}

impl<'a> ReducedChunkGraph<'a> {
  /// `atom_bits` holds the initial dependent-entry bits of each atom. No two atoms outside manual
  /// groups have equal bits. `manual_group_by_atom` (empty without groups) names the planned
  /// manual group chunk that takes each atom: the atoms of one chunk form one node, since the group
  /// puts them there whatever their bits. Such a node has no bits, so no move goes there.
  pub(super) fn new<'b>(
    atom_bits: impl IntoIterator<Item = &'b BitSet>,
    atom_dependencies: &'a [Vec<usize>],
    host_atom_by_entry_bit: &'a [Option<usize>],
    manual_group_by_atom: &[Option<usize>],
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
      only_manual_group_cycles: false,
      is_manual_group_node: Vec::with_capacity(atom_count),
      marks: Vec::with_capacity(atom_count),
      mark: 0,
    };
    let mut node_by_manual_group: FxHashMap<usize, usize> = FxHashMap::default();
    for (atom_idx, bits) in atom_bits.into_iter().enumerate() {
      let node = match manual_group_by_atom.get(atom_idx).copied().flatten() {
        Some(group) => {
          *node_by_manual_group.entry(group).or_insert_with(|| graph.add_manual_group_node())
        }
        None => {
          debug_assert!(!graph.node_by_bits.contains_key(bits), "atoms must have different bits");
          graph.add_node(bits)
        }
      };
      graph.node_by_atom.push(node);
      graph.atom_count_by_node[node] += 1;
    }
    debug_assert_eq!(graph.node_by_atom.len(), atom_count);
    for (atom_idx, dependencies) in atom_dependencies.iter().enumerate() {
      for &dependency in dependencies {
        graph.atom_dependents[dependency].push(atom_idx);
        let (from, to) = (graph.node_by_atom[atom_idx], graph.node_by_atom[dependency]);
        if from != to {
          graph.edges.add(from, to);
        }
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
    graph.is_acyclic = !graph.has_cycle(false);
    if !graph.is_acyclic {
      graph.only_manual_group_cycles = !graph.has_cycle(true);
      if !graph.only_manual_group_cycles {
        graph.cyclic_component = graph.only_cyclic_component();
      }
    }
    graph
  }

  /// Moves the atom to the node of `bits` and returns true if the move closes no chunk cycle.
  /// Otherwise, it keeps the atom where it was and returns false. See `try_move_atoms`.
  pub(super) fn try_move(&mut self, atom_idx: usize, bits: &BitSet) -> bool {
    self.try_move_atoms(&[atom_idx], bits)
  }

  /// Moves all the atoms to the node of `bits` and returns true if the chunk graph stays acyclic,
  /// or, while every cycle passes through a manual group chunk, if the moves close no new cycle.
  /// Otherwise, it keeps every atom where it was and returns false. Atoms that depend on each other
  /// can close a cycle when they move one by one and stay acyclic when they move together.
  pub(super) fn try_move_atoms(&mut self, atom_idxs: &[usize], bits: &BitSet) -> bool {
    let from_nodes: SmallVec<[usize; 1]> =
      atom_idxs.iter().map(|&atom_idx| self.node_by_atom[atom_idx]).collect();
    // Moves out of two nodes can break cycles in two components, so the shortcut needs one node.
    if !self.is_acyclic
      && !self.only_manual_group_cycles
      && from_nodes.iter().all(|&from| from == from_nodes[0])
      && !self.cyclic_component.contains(&from_nodes[0])
    {
      return false;
    }
    let to = match self.node_by_bits.get(bits) {
      Some(&node) => node,
      None => self.add_node(bits),
    };
    // All the atoms go to `to`, so every edge that the moves add still starts or ends at `to`.
    let edges_before = self.only_manual_group_cycles.then(|| self.edges_of(to));
    for (&atom_idx, &from) in atom_idxs.iter().zip(&from_nodes) {
      debug_assert_ne!(from, to, "the reduced bits must differ from the current bits");
      self.move_atom(atom_idx, from, to);
    }
    // While the graph has a cycle, the moves can remove every old cycle. They can also close new
    // cycles through `to`. So only a full check is exact. A rejected move leaves
    // `cyclic_component` correct.
    let accepted = if self.is_acyclic {
      !self.reaches_itself(to)
    } else if let Some((dependencies_before, dependents_before)) = edges_before {
      !self.adds_an_edge_on_a_cycle(to, &dependencies_before, &dependents_before)
    } else {
      !self.has_cycle(false)
    };
    if !accepted {
      for (&atom_idx, &from) in atom_idxs.iter().zip(&from_nodes).rev() {
        self.move_atom(atom_idx, to, from);
      }
    } else if !self.only_manual_group_cycles {
      self.is_acyclic = true;
    }
    accepted
  }

  fn edges_of(&self, node: usize) -> (FxHashSet<usize>, FxHashSet<usize>) {
    (self.edges.dependencies[node].keys().copied().collect(), self.edges.dependents[node].clone())
  }

  /// Whether an edge of `node` that it did not have before the move lies on a cycle. An edge from
  /// `node` to `dependency` lies on a cycle when `dependency` reaches `node`, and an edge from
  /// `dependent` to `node` when `node` reaches `dependent`.
  fn adds_an_edge_on_a_cycle(
    &mut self,
    node: usize,
    dependencies_before: &FxHashSet<usize>,
    dependents_before: &FxHashSet<usize>,
  ) -> bool {
    let added_dependencies: Vec<usize> = self.edges.dependencies[node]
      .keys()
      .copied()
      .filter(|dependency| !dependencies_before.contains(dependency))
      .collect();
    let added_dependents: Vec<usize> = self.edges.dependents[node]
      .iter()
      .copied()
      .filter(|dependent| !dependents_before.contains(dependent))
      .collect();
    added_dependencies.into_iter().any(|dependency| self.reaches(&[dependency], node))
      || added_dependents.into_iter().any(|dependent| self.reaches(&[node], dependent))
  }

  fn add_manual_group_node(&mut self) -> usize {
    let node = self.atom_count_by_node.len();
    self.atom_count_by_node.push(0);
    self.facade_entry_bit_by_node.push(None);
    self.is_manual_group_node.push(true);
    self.edges.add_node();
    self.marks.push(0);
    node
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
    self.is_manual_group_node.push(false);
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
    if self.atom_count_by_node[facade] == 0 {
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
    let dependencies: Vec<usize> = self.edges.dependencies[node].keys().copied().collect();
    self.reaches(&dependencies, node)
  }

  /// Whether a path leads from one of `starts` to `target`. A start that is `target` counts.
  fn reaches(&mut self, starts: &[usize], target: usize) -> bool {
    if self.mark == u32::MAX {
      self.marks.fill(0);
      self.mark = 0;
    }
    self.mark += 1;
    let mut stack: Vec<usize> = starts.to_vec();
    while let Some(next) = stack.pop() {
      if next == target {
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

  /// Whether the graph of the nodes with atoms has a cycle. With `skip_manual_group_nodes`, the
  /// search leaves out the manual group chunks, so it finds only cycles that avoid them.
  fn has_cycle(&self, skip_manual_group_nodes: bool) -> bool {
    const NEW: u8 = 0;
    const ON_PATH: u8 = 1;
    const DONE: u8 = 2;
    let node_count = self.atom_count_by_node.len();
    let mut state = vec![NEW; node_count];
    if skip_manual_group_nodes {
      for (node, &is_manual_group_node) in self.is_manual_group_node.iter().enumerate() {
        if is_manual_group_node {
          state[node] = DONE;
        }
      }
    }
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

  /// See `cyclic_component`. This is Kosaraju's algorithm: a post-order over the dependencies,
  /// then a search over the dependents from each node in reverse post-order. Each such search
  /// collects one component.
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

      if self.only_manual_group_cycles {
        assert!(!self.is_acyclic);
        assert!(!self.has_cycle(true));
      } else {
        assert_eq!(self.is_acyclic, !self.has_cycle(false));
        if !self.is_acyclic {
          assert_eq!(self.cyclic_component, self.only_cyclic_component());
        }
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
    let mut graph =
      ReducedChunkGraph::new(&current, atom_dependencies, host_atom_by_entry_bit, &[]);
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
  fn a_fold_through_a_manual_group_chunk_is_rejected() {
    // Entry 0 (atom 0) imports atom 1, which a manual group takes and which has the same bits, and
    // atom 1 imports atom 2, which entry 1 also loads. Without the group, atom 1 would be in the
    // chunk of entry 0, and a fold of atom 2 into entry 0 would be safe. The group puts atom 1 in
    // its own chunk, which imports atom 2 back from entry 0.
    let atom_bits = [bits(2, &[0]), bits(2, &[0]), bits(2, &[0, 1])];
    let atom_dependencies = [vec![1], vec![2], vec![]];
    let mut graph =
      ReducedChunkGraph::new(&atom_bits, &atom_dependencies, &[None; 2], &[None, Some(0), None]);
    assert!(!graph.try_move(2, &bits(2, &[0])));
    graph.assert_consistent();
  }

  #[test]
  fn a_cycle_through_a_manual_group_chunk_rejects_only_moves_that_close_a_new_cycle() {
    // Atom 0 is a manual group chunk. Entry 0 (atom 1) imports it, and it imports atom 1 back, so
    // the graph starts with that cycle. Atom 1 imports atom 2, which entry 1 also loads. A fold of
    // atom 2 into entry 0 adds no edge, so it is accepted.
    let atom_bits = [bits(2, &[0]), bits(2, &[0]), bits(2, &[0, 1])];
    let manual_groups = [Some(0), None, None];
    let atom_dependencies = [vec![1], vec![0, 2], vec![]];
    let mut graph =
      ReducedChunkGraph::new(&atom_bits, &atom_dependencies, &[None; 2], &manual_groups);
    graph.assert_consistent();
    assert!(graph.try_move(2, &bits(2, &[0])));
    graph.assert_consistent();
    // When atom 2 also imports the group chunk, a move of atom 2 to entry 1 makes entry 1 import
    // the group chunk, which reaches entry 1 through entry 0: a new cycle.
    let atom_dependencies = [vec![1], vec![0, 2], vec![0]];
    let mut graph =
      ReducedChunkGraph::new(&atom_bits, &atom_dependencies, &[None; 2], &manual_groups);
    assert!(!graph.try_move(2, &bits(2, &[1])));
    graph.assert_consistent();
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
      let mut graph = ReducedChunkGraph::new(&current, atom_dependencies, &[None; 4], &[]);
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
                  let mut graph = ReducedChunkGraph::new(&current, &dependencies, &hosts, &[]);
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
      let mut graph = ReducedChunkGraph::new(&current, &dependencies, &hosts, &[]);
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
        ReducedChunkGraph::new(&atom_bits, &atom_dependencies, &host_atom_by_entry_bit, &[]);
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

  /// A chunk of the oracle: a manual group chunk, or the chunk of a bitset.
  type ChunkKey = (Option<usize>, BitSet);

  fn chunk_keys(atom_bits: &[BitSet], manual_groups: &[Option<usize>]) -> Vec<ChunkKey> {
    atom_bits
      .iter()
      .zip(manual_groups)
      .map(|(bits, group)| match group {
        Some(group) => (Some(*group), BitSet::new(0)),
        None => (None, bits.clone()),
      })
      .collect()
  }

  fn chunk_key_edges(
    keys: &[ChunkKey],
    atom_dependencies: &[Vec<usize>],
    host_atom_by_entry_bit: &[Option<usize>],
  ) -> FxHashSet<(ChunkKey, ChunkKey)> {
    let mut edges = FxHashSet::default();
    for (atom_idx, dependencies) in atom_dependencies.iter().enumerate() {
      for &dependency in dependencies {
        if keys[atom_idx] != keys[dependency] {
          edges.insert((keys[atom_idx].clone(), keys[dependency].clone()));
        }
      }
    }
    for (entry_bit, host_atom_idx) in host_atom_by_entry_bit.iter().enumerate() {
      let facade = keys.iter().find(|(group, bits)| {
        group.is_none()
          && bits.bit_count() == 1
          && bits.index_of_one().next() == Some(u32::try_from(entry_bit).unwrap())
      });
      if let (Some(host_atom_idx), Some(facade)) = (host_atom_idx, facade) {
        if *facade != keys[*host_atom_idx] {
          edges.insert((facade.clone(), keys[*host_atom_idx].clone()));
        }
      }
    }
    edges
  }

  fn key_reaches(edges: &FxHashSet<(ChunkKey, ChunkKey)>, from: &ChunkKey, to: &ChunkKey) -> bool {
    let mut seen = FxHashSet::default();
    let mut stack = vec![from.clone()];
    while let Some(key) = stack.pop() {
      if key == *to {
        return true;
      }
      if seen.insert(key.clone()) {
        stack
          .extend(edges.iter().filter(|(edge_from, _)| *edge_from == key).map(|(_, t)| t.clone()));
      }
    }
    false
  }

  /// Whether an edge lies on a cycle, leaving out the manual group chunks when asked.
  fn key_graph_has_cycle(
    edges: &FxHashSet<(ChunkKey, ChunkKey)>,
    skip_manual_groups: bool,
  ) -> bool {
    let kept: FxHashSet<(ChunkKey, ChunkKey)> = edges
      .iter()
      .filter(|(from, to)| !skip_manual_groups || (from.0.is_none() && to.0.is_none()))
      .cloned()
      .collect();
    kept.iter().any(|(from, to)| key_reaches(&kept, to, from))
  }

  #[test]
  fn random_moves_beside_manual_group_chunks_match_an_independent_check() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    // Accepted and rejected moves while every cycle passes through a manual group chunk.
    let mut beside_group_cycles = [0_usize; 2];
    for case in 0..2000 {
      let entry_count = 1 + rng.below(4);
      let all_bits = (1..1_u32 << entry_count)
        .map(|mask| {
          let set = (0..u32::try_from(entry_count).unwrap())
            .filter(|bit| mask & (1 << bit) != 0)
            .collect::<Vec<_>>();
          bits(u32::try_from(entry_count).unwrap(), &set)
        })
        .collect::<Vec<_>>();
      let mut atom_bits = all_bits.clone();
      for idx in (1..atom_bits.len()).rev() {
        atom_bits.swap(idx, rng.below(idx + 1));
      }
      atom_bits.truncate(1 + rng.below(atom_bits.len().min(8)));
      // One to three atoms in one or two manual group chunks, after the other atoms.
      let mut manual_groups = vec![None; atom_bits.len()];
      for _ in 0..=rng.below(3) {
        atom_bits.push(all_bits[rng.below(all_bits.len())].clone());
        manual_groups.push(Some(rng.below(2)));
      }
      let atom_count = atom_bits.len();
      let edge_chance = 1 + rng.below(4);
      let atom_dependencies = (0..atom_count)
        .map(|atom_idx| {
          (0..atom_count)
            .filter(|&dependency| dependency != atom_idx && rng.below(8) < edge_chance)
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
      let host_atom_by_entry_bit = (0..entry_count)
        .map(|_| (rng.below(4) != 0).then(|| rng.below(atom_count)))
        .collect::<Vec<_>>();
      let movable: Vec<usize> =
        (0..atom_count).filter(|&atom| manual_groups[atom].is_none()).collect();

      let mut graph = ReducedChunkGraph::new(
        &atom_bits,
        &atom_dependencies,
        &host_atom_by_entry_bit,
        &manual_groups,
      );
      graph.assert_consistent();
      for step in 0..12 {
        let moved_atom = movable[rng.below(movable.len())];
        let reduced = &all_bits[rng.below(all_bits.len())];
        if *reduced == atom_bits[moved_atom] {
          continue;
        }
        let mut next = atom_bits.clone();
        next[moved_atom] = reduced.clone();
        let before = chunk_key_edges(
          &chunk_keys(&atom_bits, &manual_groups),
          &atom_dependencies,
          &host_atom_by_entry_bit,
        );
        let after = chunk_key_edges(
          &chunk_keys(&next, &manual_groups),
          &atom_dependencies,
          &host_atom_by_entry_bit,
        );
        let expected = if key_graph_has_cycle(&before, false) && !key_graph_has_cycle(&before, true)
        {
          let accepted = !after.difference(&before).any(|(from, to)| key_reaches(&after, to, from));
          beside_group_cycles[usize::from(accepted)] += 1;
          accepted
        } else {
          !key_graph_has_cycle(&after, false)
        };
        assert_eq!(graph.try_move(moved_atom, reduced), expected, "case {case}, step {step}");
        graph.assert_consistent();
        if expected {
          atom_bits = next;
        }
      }
    }
    assert!(beside_group_cycles.iter().all(|&count| count > 0), "{beside_group_cycles:?}");
  }
}
