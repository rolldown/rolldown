use rolldown_utils::BitSet;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

/// The chunk graph that the atoms' current dependent-entry bits produce, kept up to date while
/// `optimize_dynamic_entry_bits` moves atoms to reduced bits. Atoms with equal bits form one node
/// (a chunk). A node depends on another node when one of its atoms depends on an atom of the
/// other node, and through the facade edges (see `facade_edge`).
///
/// While the graph is acyclic, it also keeps its nodes in a topological order. Only the node that
/// receives the atom gets new edges, so a move can only close a cycle through a new edge that
/// points backwards in the order. Besides one hash lookup of the new bits, a move without such an
/// edge costs O(atom degree), and any other move searches only the nodes between the backward
/// edges and the receiving node. A move only removes edges of the node that the atom leaves, so
/// while the graph has a cycle, only a move out of a node on every cycle can make the graph
/// acyclic, and only such a move needs a full check.
/// See internal-docs/code-splitting/implementation.md#dynamic-already-loaded-analysis.
pub(super) struct ReducedChunkGraph<'a> {
  atom_dependencies: &'a [Vec<usize>],
  atom_dependents: Vec<Vec<usize>>,
  host_atom_by_entry_bit: &'a [Option<usize>],
  hosted_entry_bits_by_atom: Vec<SmallVec<[usize; 1]>>,
  node_by_bits: FxHashMap<BitSet, usize>,
  node_by_atom: Vec<usize>,
  /// A node without atoms is not a chunk: it has no edges and no place in `order`.
  atom_count_by_node: Vec<usize>,
  /// The entry bit of a node whose bits hold only that entry, so the node is that entry's chunk.
  facade_entry_bit_by_node: Vec<Option<usize>>,
  facade_node_by_entry_bit: Vec<Option<usize>>,
  edges: NodeEdges,
  /// `None` while the graph has a cycle.
  order: Option<TopologicalOrder>,
  /// While the graph has a cycle: the nodes of its only strongly connected component with more
  /// than one node, or no nodes if it has more such components. Each cycle lies in one such
  /// component, and a move removes only edges of the node that the atom leaves. So a move out of a
  /// node outside this set keeps a cycle.
  cyclic_component: FxHashSet<usize>,
  /// The node edges that the current move creates. Every one of them touches the receiving node.
  new_edges: Vec<(usize, usize)>,
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
      order: None,
      cyclic_component: FxHashSet::default(),
      new_edges: vec![],
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
    graph.order = graph.topological_order();
    if graph.order.is_none() {
      graph.cyclic_component = graph.only_cyclic_component();
    }
    graph
  }

  /// Moves the atom to the node of `bits` and returns true if the chunk graph stays acyclic.
  /// Otherwise, it keeps the atom where it was and returns false.
  pub(super) fn try_move(&mut self, atom_idx: usize, bits: &BitSet) -> bool {
    let from = self.node_by_atom[atom_idx];
    if self.order.is_none() && !self.cyclic_component.contains(&from) {
      return false;
    }
    let to = match self.node_by_bits.get(bits) {
      Some(&node) => node,
      None => self.add_node(bits),
    };
    debug_assert_ne!(from, to, "the reduced bits must differ from the current bits");
    let to_was_empty = self.atom_count_by_node[to] == 0;
    self.new_edges.clear();
    self.move_atom(atom_idx, from, to);

    let moved = if let Some(mut order) = self.order.take() {
      if to_was_empty {
        // The atom's edges fit the place of `from` in the order, so `to` starts next to it.
        order.insert_after(from, to);
      }
      let moved = self.reorder(&mut order, to);
      if moved && self.atom_count_by_node[from] == 0 {
        order.remove(from);
      } else if !moved && to_was_empty {
        order.remove(to);
      }
      self.order = Some(order);
      moved
    } else {
      // The move can remove every old cycle, but it can also close new cycles through `to`, so
      // only a full check is exact. A rejected move leaves `cyclic_component` correct.
      self.order = self.topological_order();
      self.order.is_some()
    };
    if !moved {
      self.move_atom(atom_idx, to, from);
    }
    moved
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
    if let Some(order) = &mut self.order {
      order.add_node();
    }
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

  /// Moves the atom and its edges from node `from` to node `to`, and records the node edges that
  /// the move creates in `new_edges`.
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
      if node != to && self.edges.add(to, node) {
        self.new_edges.push((to, node));
      }
    }
    for &dependent in &self.atom_dependents[atom_idx] {
      let node = self.node_by_atom[dependent];
      if node != to && self.edges.add(node, to) {
        self.new_edges.push((node, to));
      }
    }
    for &entry_bit in &entry_bits {
      if let Some((facade, host)) = self.facade_edge(entry_bit) {
        if self.edges.add(facade, host) {
          self.new_edges.push((facade, host));
        }
      }
    }
  }

  /// Puts `node`, which received the moved atom, in a valid place of `order`. `order` is valid
  /// for all edges except `new_edges`, which all touch `node`. Dependents after `node` move in
  /// front of it, with all nodes after `node` that reach them. Dependencies before `node` move
  /// behind it, with all nodes before `node` that they reach. Returns false and keeps `order` as
  /// it was if the new edges close a cycle.
  fn reorder(&mut self, order: &mut TopologicalOrder, node: usize) -> bool {
    let place = order.label(node);
    if self.mark >= u32::MAX - 2 {
      self.marks.fill(0);
      self.mark = 0;
    }
    self.mark += 2;
    let (later_mark, earlier_mark) = (self.mark - 1, self.mark);

    let mut later_dependents = vec![];
    let mut earlier_dependencies = vec![];
    for &(from, to) in &self.new_edges {
      if to == node && order.label(from) > place && self.marks[from] != later_mark {
        self.marks[from] = later_mark;
        later_dependents.push(from);
      } else if from == node && order.label(to) < place && self.marks[to] != earlier_mark {
        self.marks[to] = earlier_mark;
        earlier_dependencies.push(to);
      }
    }
    if later_dependents.is_empty() && earlier_dependencies.is_empty() {
      return true;
    }

    // In a valid order, a path between two nodes after `node` stays after `node`, and a path
    // between two nodes before `node` stays before it. So a cycle through `node` exists exactly
    // when `node` reaches a later dependent, an earlier dependency reaches `node`, or an earlier
    // dependency reaches a later dependent.
    let mut idx = 0;
    while let Some(&dependent) = later_dependents.get(idx) {
      idx += 1;
      if self.edges.dependencies[node].contains_key(&dependent) {
        return false;
      }
      for &next in &self.edges.dependents[dependent] {
        if order.label(next) > place && self.marks[next] != later_mark {
          self.marks[next] = later_mark;
          later_dependents.push(next);
        }
      }
    }
    let mut idx = 0;
    while let Some(&dependency) = earlier_dependencies.get(idx) {
      idx += 1;
      if self.edges.dependents[node].contains(&dependency) {
        return false;
      }
      for &next in self.edges.dependencies[dependency].keys() {
        if order.label(next) > place {
          if self.marks[next] == later_mark {
            return false;
          }
        } else if self.marks[next] != earlier_mark {
          self.marks[next] = earlier_mark;
          earlier_dependencies.push(next);
        }
      }
    }

    later_dependents.sort_unstable_by_key(|&node| order.label(node));
    earlier_dependencies.sort_unstable_by_key(|&node| order.label(node));
    order.move_before(node, &later_dependents);
    order.move_after(node, &earlier_dependencies);
    true
  }

  /// The nodes with atoms in topological order, or `None` if the graph has a cycle. A later root
  /// comes first, so atoms of later dynamic entries come before the chunks of the entries that
  /// load them, and a fold into such a chunk seldom adds a backward edge.
  fn topological_order(&self) -> Option<TopologicalOrder> {
    const NEW: u8 = 0;
    const ON_PATH: u8 = 1;
    const DONE: u8 = 2;
    let node_count = self.atom_count_by_node.len();
    let mut state = vec![NEW; node_count];
    let mut post_order = Vec::with_capacity(node_count);
    let mut stack = vec![];
    for root in 0..node_count {
      if self.atom_count_by_node[root] == 0 || state[root] != NEW {
        continue;
      }
      stack.push((root, false));
      while let Some((node, exiting)) = stack.pop() {
        if exiting {
          state[node] = DONE;
          post_order.push(node);
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
            ON_PATH => return None,
            _ => {}
          }
        }
      }
    }
    post_order.reverse();
    Some(TopologicalOrder::new(post_order, node_count))
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
  /// Per node: the dependency nodes and the number of atom and facade edges behind each one.
  dependencies: Vec<FxHashMap<usize, u32>>,
  dependents: Vec<FxHashSet<usize>>,
}

impl NodeEdges {
  fn add_node(&mut self) {
    self.dependencies.push(FxHashMap::default());
    self.dependents.push(FxHashSet::default());
  }

  /// Adds one edge behind the node edge and returns true if the node edge is new.
  fn add(&mut self, from: usize, to: usize) -> bool {
    let count = self.dependencies[from].entry(to).or_insert(0);
    *count += 1;
    if *count > 1 {
      return false;
    }
    self.dependents[to].insert(from);
    true
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

const NO_NODE: usize = usize::MAX;

/// A doubly linked list of nodes whose labels increase along the list, so two nodes compare by
/// label in O(1), and a node moves in O(1) until the gap between two labels runs out.
struct TopologicalOrder {
  prev: Vec<usize>,
  next: Vec<usize>,
  labels: Vec<u64>,
  head: usize,
  len: usize,
}

impl TopologicalOrder {
  fn new(nodes: Vec<usize>, node_count: usize) -> Self {
    let mut order = Self {
      prev: vec![NO_NODE; node_count],
      next: vec![NO_NODE; node_count],
      labels: vec![0; node_count],
      head: NO_NODE,
      len: 0,
    };
    let mut prev = NO_NODE;
    for node in nodes {
      order.link_after(prev, node);
      prev = node;
    }
    order.relabel();
    order
  }

  fn add_node(&mut self) {
    self.prev.push(NO_NODE);
    self.next.push(NO_NODE);
    self.labels.push(0);
  }

  fn label(&self, node: usize) -> u64 {
    self.labels[node]
  }

  fn insert_after(&mut self, anchor: usize, node: usize) {
    self.link_after(anchor, node);
    self.label_after(anchor, 1);
  }

  fn remove(&mut self, node: usize) {
    let (prev, next) = (self.prev[node], self.next[node]);
    if prev == NO_NODE {
      self.head = next;
    } else {
      self.next[prev] = next;
    }
    if next != NO_NODE {
      self.prev[next] = prev;
    }
    self.prev[node] = NO_NODE;
    self.next[node] = NO_NODE;
    self.len -= 1;
  }

  /// Moves `nodes`, which are sorted by label, to the place right before `anchor`.
  fn move_before(&mut self, anchor: usize, nodes: &[usize]) {
    if nodes.is_empty() {
      return;
    }
    for &node in nodes {
      self.remove(node);
    }
    let first_prev = self.prev[anchor];
    let mut prev = first_prev;
    for &node in nodes {
      self.link_after(prev, node);
      prev = node;
    }
    self.label_after(first_prev, nodes.len());
  }

  /// Moves `nodes`, which are sorted by label, to the place right after `anchor`.
  fn move_after(&mut self, anchor: usize, nodes: &[usize]) {
    if nodes.is_empty() {
      return;
    }
    for &node in nodes {
      self.remove(node);
    }
    let mut prev = anchor;
    for &node in nodes {
      self.link_after(prev, node);
      prev = node;
    }
    self.label_after(anchor, nodes.len());
  }

  /// Links `node` after `prev`, or at the head for `NO_NODE`.
  fn link_after(&mut self, prev: usize, node: usize) {
    let next = if prev == NO_NODE { self.head } else { self.next[prev] };
    self.prev[node] = prev;
    self.next[node] = next;
    if prev == NO_NODE {
      self.head = node;
    } else {
      self.next[prev] = node;
    }
    if next != NO_NODE {
      self.prev[next] = node;
    }
    self.len += 1;
  }

  /// Labels the `count` nodes after `prev` (or at the head for `NO_NODE`) evenly between the
  /// labels of their neighbors. It relabels the full list if the gap is too small.
  fn label_after(&mut self, prev: usize, count: usize) {
    let first = if prev == NO_NODE { self.head } else { self.next[prev] };
    let mut after = first;
    for _ in 0..count {
      after = self.next[after];
    }
    let low = if prev == NO_NODE { 0 } else { self.labels[prev] };
    let high = if after == NO_NODE { u64::MAX } else { self.labels[after] };
    let step = (high - low) / (count as u64 + 1);
    if step == 0 {
      self.relabel();
      return;
    }
    let (mut node, mut label) = (first, low);
    for _ in 0..count {
      label += step;
      self.labels[node] = label;
      node = self.next[node];
    }
  }

  fn relabel(&mut self) {
    let step = u64::MAX / (self.len as u64 + 1);
    let (mut node, mut label) = (self.head, 0);
    while node != NO_NODE {
      label += step;
      self.labels[node] = label;
      node = self.next[node];
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  impl ReducedChunkGraph<'_> {
    /// Recomputes the edges from the atoms and checks the order against them.
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

      let Some(order) = &self.order else {
        assert_eq!(self.cyclic_component, self.only_cyclic_component());
        return;
      };
      let mut position = vec![None; node_count];
      let (mut node, mut prev, mut idx) = (order.head, NO_NODE, 0);
      while node != NO_NODE {
        assert_eq!(order.prev[node], prev);
        if prev != NO_NODE {
          assert!(order.label(prev) < order.label(node));
        }
        position[node] = Some(idx);
        (prev, node, idx) = (node, order.next[node], idx + 1);
      }
      assert_eq!(order.len, idx);
      for (node, &atom_count) in self.atom_count_by_node.iter().enumerate() {
        assert_eq!(position[node].is_some(), atom_count > 0, "only nodes with atoms are ordered");
      }
      for (from, node_dependencies) in dependencies.iter().enumerate() {
        for &to in node_dependencies.keys() {
          assert!(position[from] < position[to], "edge {from} -> {to} points backwards");
        }
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
    let mut state = vec![0; dependencies.len()];
    (0..dependencies.len())
      .any(|chunk| state[chunk] == 0 && reaches_cycle(chunk, &dependencies, &mut state))
  }

  fn reaches_cycle(chunk: usize, dependencies: &[FxHashSet<usize>], state: &mut [u8]) -> bool {
    state[chunk] = 1;
    for &dependency in &dependencies[chunk] {
      if state[dependency] == 1
        || (state[dependency] == 0 && reaches_cycle(dependency, dependencies, state))
      {
        return true;
      }
    }
    state[chunk] = 2;
    false
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
    // cycle is rejected. Merging the two atoms removes the cycle, and the order exists again.
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
  fn a_backward_edge_reorders_the_nodes_it_passes() {
    // The initial order is [3, 2, 1, 0]. Moving atom 0 into atom 3's chunk gives it the edge
    // 1 -> 3 that points backwards, so 1 and its dependent 2 move in front of 3. A later move
    // that needs the new order then closes a cycle.
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

  #[test]
  fn random_moves_match_a_full_cycle_check() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..4000 {
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
      // Half of the graphs start acyclic, so that their moves use the order from the start.
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
      for _ in 0..16 {
        let atom_idx = rng.below(atom_count);
        let reduced = &all_bits[rng.below(all_bits.len())];
        if *reduced == atom_bits[atom_idx] {
          continue;
        }
        let mut next = atom_bits.clone();
        next[atom_idx] = reduced.clone();
        let expected = !has_cycle(&next, &atom_dependencies, &host_atom_by_entry_bit);
        assert_eq!(graph.try_move(atom_idx, reduced), expected);
        graph.assert_consistent();
        if expected {
          atom_bits = next;
        }
      }
    }
  }
}
