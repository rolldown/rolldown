//! Dominator-tree analysis over the module graph: retained-size attribution.
//!
//! For every module reachable from the user-defined entries over STATIC edges (everything
//! except `dynamic-import` — the same "initial load" notion as `EntrySection`), compute its
//! immediate dominator and the bytes of the dominator subtree it roots. `retained_bytes[m]`
//! answers the question every load-time optimization starts with: *if the import edge that
//! pulls `m` in were deferred, how many bytes would leave the initial load?* — the answer is
//! exactly the subtree `m` dominates (the same accounting heap snapshots use for retained
//! size, and the module-graph analog of SimplifyGraph's CFG region analysis).
//!
//! Dominators come from the iterative Cooper–Harvey–Kennedy algorithm ("A Simple, Fast
//! Dominance Algorithm") over a virtual root whose successors are the entry modules; module
//! graphs are small enough that its simplicity beats Lengauer-Tarjan's constants.

use rustc_hash::FxHashMap;

/// One analyzed module. `imports` targets are indices into [`GraphAnalysis::nodes`].
pub(crate) struct GraphNode {
  /// Raw module id (stabilize for display at the report layer).
  pub id: String,
  /// Rendered bytes (0 for externals / modules absent from `ModuleRenderedReady`).
  pub bytes: u64,
  /// Immediate dominator as a node index; `None` for a module hanging directly off the
  /// virtual root (an entry module, or a join point shared by several entries) — and for
  /// modules that are not statically reachable at all (`static_reachable == false`).
  pub idom: Option<usize>,
  pub static_reachable: bool,
  /// Reachable from the entries, but only across at least one `dynamic-import` edge — its
  /// bytes are NOT part of any initial load.
  pub dynamic_only: bool,
  /// Total bytes of the dominator subtree rooted here (own bytes included).
  pub retained_bytes: u64,
  /// Module count of that subtree (self included).
  pub retained_count: usize,
  /// Outgoing edges as `(target node index, is_dynamic)`, deduplicated, sorted.
  pub imports: Vec<(usize, bool)>,
}

pub(crate) struct GraphAnalysis {
  /// Raw ids of the user-defined entry modules the virtual root points at.
  pub entry_modules: Vec<String>,
  /// Node table, sorted by raw id (deterministic across builds).
  pub nodes: Vec<GraphNode>,
  pub static_module_count: usize,
  pub static_bytes: u64,
  pub dynamic_only_count: usize,
}

/// Run the analysis. `adjacency` maps module id -> `(imported id, is_dynamic)` edges;
/// `bytes` maps module id -> rendered bytes; `entries` are the user-defined entry module ids.
/// Returns `None` when there is nothing to analyze (no entries or no modules).
pub(crate) fn analyze(
  adjacency: &FxHashMap<String, Vec<(String, bool)>>,
  bytes: &FxHashMap<String, u64>,
  entries: &[&str],
) -> Option<GraphAnalysis> {
  // --- node universe: every id seen as a source, a target, a weight, or an entry ---
  let mut ids: Vec<&str> = adjacency
    .iter()
    .flat_map(|(from, deps)| {
      std::iter::once(from.as_str()).chain(deps.iter().map(|(to, _)| to.as_str()))
    })
    .chain(bytes.keys().map(String::as_str))
    .chain(entries.iter().copied())
    .collect();
  ids.sort_unstable();
  ids.dedup();
  if ids.is_empty() || entries.is_empty() {
    return None;
  }
  let index_of: FxHashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
  let n = ids.len();

  // --- edge lists (deduped, sorted for deterministic traversal order) ---
  let mut imports: Vec<Vec<(usize, bool)>> = vec![Vec::new(); n];
  for (from, deps) in adjacency {
    let from_idx = index_of[from.as_str()];
    let list = &mut imports[from_idx];
    for (to, is_dynamic) in deps {
      list.push((index_of[to.as_str()], *is_dynamic));
    }
    list.sort_unstable();
    list.dedup();
  }
  let entry_idxs: Vec<usize> = {
    let mut e: Vec<usize> = entries.iter().map(|id| index_of[id]).collect();
    e.sort_unstable();
    e.dedup();
    e
  };

  // --- reverse post-order over STATIC edges from a virtual root (index n) ---
  // Iterative DFS with an explicit "children pending" state so the post-order is exact.
  let virtual_root = n;
  let succs_static = |v: usize| -> Vec<usize> {
    if v == virtual_root {
      entry_idxs.clone()
    } else {
      imports[v].iter().filter(|(_, dynamic)| !dynamic).map(|(to, _)| *to).collect()
    }
  };
  let mut post_order: Vec<usize> = Vec::with_capacity(n + 1);
  {
    let mut visited = vec![false; n + 1];
    // (node, next-successor cursor)
    let mut stack: Vec<(usize, usize)> = vec![(virtual_root, 0)];
    visited[virtual_root] = true;
    while let Some((v, cursor)) = stack.pop() {
      let succs = succs_static(v);
      if let Some(&next) = succs.get(cursor) {
        stack.push((v, cursor + 1));
        if !visited[next] {
          visited[next] = true;
          stack.push((next, 0));
        }
      } else {
        post_order.push(v);
      }
    }
  }
  // rpo_number[v]: position in reverse post-order; usize::MAX = statically unreachable.
  let mut rpo_number = vec![usize::MAX; n + 1];
  for (i, &v) in post_order.iter().rev().enumerate() {
    rpo_number[v] = i;
  }
  let static_reachable: Vec<bool> = (0..n).map(|v| rpo_number[v] != usize::MAX).collect();

  // --- static predecessors (only among reachable nodes; entries also get the virtual root) ---
  let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n + 1];
  for from in 0..n {
    if !static_reachable[from] {
      continue;
    }
    for &(to, is_dynamic) in &imports[from] {
      if !is_dynamic {
        preds[to].push(from);
      }
    }
  }
  for &e in &entry_idxs {
    preds[e].push(virtual_root);
  }

  // --- Cooper–Harvey–Kennedy iterative dominators ---
  let mut idom = vec![usize::MAX; n + 1];
  idom[virtual_root] = virtual_root;
  let intersect = |idom: &[usize], rpo: &[usize], mut a: usize, mut b: usize| -> usize {
    while a != b {
      while rpo[a] > rpo[b] {
        a = idom[a];
      }
      while rpo[b] > rpo[a] {
        b = idom[b];
      }
    }
    a
  };
  let order: Vec<usize> = post_order.iter().rev().copied().filter(|&v| v != virtual_root).collect();
  let mut changed = true;
  while changed {
    changed = false;
    for &v in &order {
      let mut new_idom = usize::MAX;
      for &p in &preds[v] {
        if idom[p] == usize::MAX {
          continue; // predecessor not processed yet
        }
        new_idom =
          if new_idom == usize::MAX { p } else { intersect(&idom, &rpo_number, new_idom, p) };
      }
      if new_idom != usize::MAX && idom[v] != new_idom {
        idom[v] = new_idom;
        changed = true;
      }
    }
  }

  // --- retained sizes: fold every subtree into its dominator, leaves first ---
  // In reverse post-order, idom[v] always precedes v, so iterating post-order (children of the
  // dominator tree cannot precede their parent in RPO) and pushing each completed node into its
  // parent accumulates exact subtree sums.
  let node_bytes: Vec<u64> = (0..n).map(|v| bytes.get(ids[v]).copied().unwrap_or(0)).collect();
  let mut retained_bytes: Vec<u64> =
    (0..n + 1).map(|v| if v < n && static_reachable[v] { node_bytes[v] } else { 0 }).collect();
  let mut retained_count: Vec<usize> =
    (0..n + 1).map(|v| usize::from(v < n && static_reachable[v])).collect();
  for &v in &post_order {
    if v == virtual_root || idom[v] == usize::MAX || idom[v] == v {
      continue;
    }
    let (bytes_v, count_v) = (retained_bytes[v], retained_count[v]);
    retained_bytes[idom[v]] += bytes_v;
    retained_count[idom[v]] += count_v;
  }

  // --- dynamic-only: reachable over ALL edges, but not statically ---
  let mut reachable_all = vec![false; n];
  {
    let mut stack: Vec<usize> = entry_idxs.clone();
    for &e in &entry_idxs {
      reachable_all[e] = true;
    }
    while let Some(v) = stack.pop() {
      for &(to, _) in &imports[v] {
        if !reachable_all[to] {
          reachable_all[to] = true;
          stack.push(to);
        }
      }
    }
  }

  let mut static_module_count = 0usize;
  let mut static_bytes = 0u64;
  let mut dynamic_only_count = 0usize;
  let nodes: Vec<GraphNode> = (0..n)
    .map(|v| {
      let reachable = static_reachable[v];
      let dynamic_only = reachable_all[v] && !reachable;
      if reachable {
        static_module_count += 1;
        static_bytes += node_bytes[v];
      }
      if dynamic_only {
        dynamic_only_count += 1;
      }
      GraphNode {
        id: ids[v].to_string(),
        bytes: node_bytes[v],
        idom: (reachable && idom[v] != usize::MAX && idom[v] != virtual_root)
          .then(|| idom[v])
          .filter(|&d| d < n),
        static_reachable: reachable,
        dynamic_only,
        retained_bytes: if reachable { retained_bytes[v] } else { 0 },
        retained_count: if reachable { retained_count[v] } else { 0 },
        imports: imports[v].clone(),
      }
    })
    .collect();

  let entry_modules = entry_idxs.iter().map(|&e| ids[e].to_string()).collect();
  Some(GraphAnalysis {
    entry_modules,
    nodes,
    static_module_count,
    static_bytes,
    dynamic_only_count,
  })
}

#[cfg(test)]
mod unit {
  use super::*;

  fn adj(edges: &[(&str, &str, bool)]) -> FxHashMap<String, Vec<(String, bool)>> {
    let mut map: FxHashMap<String, Vec<(String, bool)>> = FxHashMap::default();
    for (from, to, dynamic) in edges {
      map.entry((*from).to_string()).or_default().push(((*to).to_string(), *dynamic));
    }
    map
  }

  fn weights(rows: &[(&str, u64)]) -> FxHashMap<String, u64> {
    rows.iter().map(|(id, b)| ((*id).to_string(), *b)).collect()
  }

  fn node<'a>(analysis: &'a GraphAnalysis, id: &str) -> &'a GraphNode {
    analysis.nodes.iter().find(|n| n.id == id).unwrap()
  }

  #[test]
  fn diamond_retains_join_under_fork() {
    // entry -> a -> c, entry -> b -> c: c is dominated by entry (join), not by a or b.
    let analysis = analyze(
      &adj(&[("entry", "a", false), ("entry", "b", false), ("a", "c", false), ("b", "c", false)]),
      &weights(&[("entry", 10), ("a", 100), ("b", 200), ("c", 1000)]),
      &["entry"],
    )
    .unwrap();
    assert_eq!(node(&analysis, "a").retained_bytes, 100);
    assert_eq!(node(&analysis, "b").retained_bytes, 200);
    assert_eq!(
      node(&analysis, "c").idom,
      Some(analysis.nodes.iter().position(|n| n.id == "entry").unwrap())
    );
    assert_eq!(node(&analysis, "entry").retained_bytes, 1310);
    assert_eq!(node(&analysis, "entry").idom, None); // hangs off the virtual root
  }

  #[test]
  fn chain_retains_transitively_and_dynamic_cuts() {
    // entry -> route -> heavy -> util, entry -(dynamic)-> lazy -> lazydep
    let analysis = analyze(
      &adj(&[
        ("entry", "route", false),
        ("route", "heavy", false),
        ("heavy", "util", false),
        ("entry", "lazy", true),
        ("lazy", "lazydep", false),
      ]),
      &weights(&[
        ("entry", 5),
        ("route", 10),
        ("heavy", 5000),
        ("util", 300),
        ("lazy", 70),
        ("lazydep", 30),
      ]),
      &["entry"],
    )
    .unwrap();
    // Deferring `route` frees route + heavy + util.
    assert_eq!(node(&analysis, "route").retained_bytes, 5310);
    assert_eq!(node(&analysis, "route").retained_count, 3);
    // The dynamic branch is not part of the static view at all.
    assert!(node(&analysis, "lazy").dynamic_only);
    assert!(node(&analysis, "lazydep").dynamic_only);
    assert_eq!(node(&analysis, "lazy").retained_bytes, 0);
    assert_eq!(analysis.static_bytes, 5315);
    assert_eq!(analysis.static_module_count, 4);
    assert_eq!(analysis.dynamic_only_count, 2);
  }

  #[test]
  fn multi_entry_shared_module_joins_at_root() {
    // Two entries both import shared: shared hangs off the virtual root (idom None),
    // so neither entry's retained size claims it.
    let analysis = analyze(
      &adj(&[("a", "shared", false), ("b", "shared", false)]),
      &weights(&[("a", 10), ("b", 20), ("shared", 500)]),
      &["a", "b"],
    )
    .unwrap();
    assert_eq!(node(&analysis, "shared").idom, None);
    assert_eq!(node(&analysis, "a").retained_bytes, 10);
    assert_eq!(node(&analysis, "b").retained_bytes, 20);
  }

  #[test]
  fn cycle_inside_dominated_region_stays_retained() {
    // entry -> a <-> b (cycle): both dominated by a's edge... a dominates b even with the
    // back edge, and the pair's bytes fold into a.
    let analysis = analyze(
      &adj(&[("entry", "a", false), ("a", "b", false), ("b", "a", false)]),
      &weights(&[("entry", 1), ("a", 40), ("b", 60)]),
      &["entry"],
    )
    .unwrap();
    assert_eq!(node(&analysis, "a").retained_bytes, 100);
    assert_eq!(node(&analysis, "b").retained_bytes, 60);
  }
}
