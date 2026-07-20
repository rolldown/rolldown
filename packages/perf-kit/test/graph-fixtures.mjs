// Shared test helper (not a test file itself — it registers no tests). Builds a
// sidecar-shaped graph from a tiny node list, computing the SAME dominator fields
// (idom / retainedBytes / retainedModuleCount / staticReachable / dynamicOnly) the
// Rust producer emits, via an independent Cooper-Harvey-Kennedy pass. Two independent
// methods (this dominator fold vs the overlay's forward-reachability difference)
// computing retainedBytes cross-check each other and the Rust producer's semantics.

import { computeStaticPreds } from '../lib/module-graph.mjs';

export const indexOf = (graph, id) => graph.modules.findIndex((m) => m.id === id);

// nodes: [{ id, bytes, imports: [[toIndex, isDynamic], ...] }]; entryIds: string[]
export function makeGraph(nodes, entryIds) {
  const n = nodes.length;
  const idToIndex = new Map(nodes.map((m, i) => [m.id, i]));
  const entryIdxs = entryIds.map((id) => idToIndex.get(id));
  const root = n;
  const staticSuccs = (v) =>
    v === root ? entryIdxs : (nodes[v].imports ?? []).filter(([, dyn]) => !dyn).map(([to]) => to);

  // reverse post-order over static edges from the virtual root
  const postOrder = [];
  const visited = new Array(n + 1).fill(false);
  const stack = [[root, 0]];
  visited[root] = true;
  while (stack.length) {
    const top = stack[stack.length - 1];
    const succs = staticSuccs(top[0]);
    if (top[1] < succs.length) {
      const next = succs[top[1]];
      top[1] += 1;
      if (!visited[next]) {
        visited[next] = true;
        stack.push([next, 0]);
      }
    } else {
      postOrder.push(top[0]);
      stack.pop();
    }
  }
  const rpo = new Array(n + 1).fill(Infinity);
  postOrder
    .slice()
    .reverse()
    .forEach((v, i) => {
      rpo[v] = i;
    });
  const staticReachable = (v) => rpo[v] !== Infinity;

  const preds = Array.from({ length: n + 1 }, () => []);
  for (let from = 0; from < n; from++) {
    if (!staticReachable(from)) continue;
    for (const [to, dyn] of nodes[from].imports ?? []) if (!dyn) preds[to].push(from);
  }
  for (const e of entryIdxs) preds[e].push(root);

  const idom = new Array(n + 1).fill(-1);
  idom[root] = root;
  const intersect = (a0, b0) => {
    let a = a0;
    let b = b0;
    while (a !== b) {
      while (rpo[a] > rpo[b]) a = idom[a];
      while (rpo[b] > rpo[a]) b = idom[b];
    }
    return a;
  };
  const order = postOrder
    .slice()
    .reverse()
    .filter((v) => v !== root);
  let changed = true;
  while (changed) {
    changed = false;
    for (const v of order) {
      let newIdom = -1;
      for (const p of preds[v]) {
        if (idom[p] === -1) continue;
        newIdom = newIdom === -1 ? p : intersect(newIdom, p);
      }
      if (newIdom !== -1 && idom[v] !== newIdom) {
        idom[v] = newIdom;
        changed = true;
      }
    }
  }

  const retainedBytes = new Array(n + 1).fill(0);
  const retainedCount = new Array(n + 1).fill(0);
  for (let v = 0; v < n; v++) {
    if (staticReachable(v)) {
      retainedBytes[v] = nodes[v].bytes;
      retainedCount[v] = 1;
    }
  }
  for (const v of postOrder) {
    if (v === root || idom[v] === -1 || idom[v] === v) continue;
    retainedBytes[idom[v]] += retainedBytes[v];
    retainedCount[idom[v]] += retainedCount[v];
  }

  const reachAll = new Array(n).fill(false);
  const s2 = [...entryIdxs];
  for (const e of entryIdxs) reachAll[e] = true;
  while (s2.length) {
    const v = s2.pop();
    for (const [to] of nodes[v].imports ?? [])
      if (!reachAll[to]) {
        reachAll[to] = true;
        s2.push(to);
      }
  }

  const modules = nodes.map((m, v) => {
    const reach = staticReachable(v);
    return {
      id: m.id,
      bytes: m.bytes,
      imports: m.imports ?? [],
      staticReachable: reach,
      dynamicOnly: reachAll[v] && !reach,
      idom: reach && idom[v] !== -1 && idom[v] !== root && idom[v] < n ? idom[v] : null,
      retainedBytes: reach ? retainedBytes[v] : 0,
      retainedModuleCount: reach ? retainedCount[v] : 0,
    };
  });
  return { entryModules: entryIds, modules, staticPreds: computeStaticPreds(modules) };
}

// A straight chain entry -> n0 -> n1 -> ... -> n{depth}. Used to prove the iterative
// solver survives a graph tens of thousands of levels deep (recursion would overflow).
export function chain(depth) {
  const nodes = [];
  for (let i = 0; i <= depth; i++) {
    nodes.push({ id: `n${i}`, bytes: 1, imports: i < depth ? [[i + 1, false]] : [] });
  }
  return makeGraph(nodes, ['n0']);
}
