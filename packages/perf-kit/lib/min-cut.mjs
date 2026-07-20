// Minimum edge cut over the static module graph: the smallest set of import statements
// to make dynamic so a target module leaves the initial load. Dominator retained-size
// can't answer this once a module is reachable by several independent paths — a cut can.
//
// Unit-capacity max-flow / min-cut (Menger): S --INF--> each entry, every static edge
// --1--> (so max flow = min number of import edges to cut), target --INF--> T. Protected
// edges get INF so the cut routes around them. We return the cut NEAREST the sink (the
// last-hop imports into the feature — the exact files to edit), computed from the sink
// side of the residual graph. Reimplemented fresh from unigraph's min_cut.rs semantics;
// fully iterative (explicit stacks/queues) — dependency graphs reach 50k modules and
// tens of thousands of levels deep, which would overflow a recursive DFS.

// Sentinel capacity for the artificial/protected edges. Real max flow is bounded by the
// number of unit-capacity edges, so this is never a bottleneck on a cuttable path.
const INF = 1e12;

/** Entry module indices (the flow sources). */
function entryIdx(graph) {
  const idOf = new Map(graph.modules.map((m, i) => [m.id, i]));
  return (graph.entryModules ?? []).map((id) => idOf.get(id)).filter((i) => i != null);
}

/**
 * Minimum edge cut separating `target` from the entries over static edges, never cutting
 * an edge in `protectedEdges` (`{from,to}` pairs — e.g. every import into a `--keep`
 * sentry). Returns `{ cutEdges:[{from,to}], flow, hasUncuttableSink, blockedByProtected }`.
 * `flow === cutEdges.length` is asserted — the built-in min-cut sanity check.
 */
export function minCut(graph, target, protectedEdges = [], sourcesArg = null) {
  const mods = graph.modules;
  const n = mods.length;
  const sources = sourcesArg ?? entryIdx(graph);

  // A target that is itself an entry can't be detached by cutting imports — you'd have
  // to delete the module. Flag it, don't cut.
  if (sources.includes(target)) {
    return { cutEdges: [], flow: 0, hasUncuttableSink: true, blockedByProtected: false };
  }

  const source = n;
  const sink = n + 1;
  const total = n + 2;

  // Flow network: forward/backward edges stored as consecutive pairs, so reverse(e) = e^1.
  const edgeTo = [];
  const edgeCap = [];
  const adjacency = Array.from({ length: total }, () => []);
  const realEdges = []; // cuttable graph edges we added, in insertion order
  const addEdge = (from, to, cap) => {
    adjacency[from].push(edgeTo.length);
    edgeTo.push(to);
    edgeCap.push(cap);
    adjacency[to].push(edgeTo.length);
    edgeTo.push(from);
    edgeCap.push(0);
  };

  for (const s of sources) addEdge(source, s, INF);
  const targetSinkFwd = edgeTo.length;
  addEdge(target, sink, INF);

  const protectedSet = new Set(protectedEdges.map((e) => e.from * n + e.to));
  // Duplicate parallel records (the same from->to listed twice in a sidecar) collapse to
  // ONE unit edge: the cut is counted in import statements at edge-pair granularity, and
  // two parallel unit edges would push flow past the deduped cut list (invariant throw).
  const seenEdge = new Set();
  for (let from = 0; from < n; from++) {
    if (!mods[from].staticReachable) continue; // unreachable nodes carry no flow
    for (const [to, isDynamic] of mods[from].imports ?? []) {
      if (isDynamic) continue;
      const key = from * n + to;
      if (seenEdge.has(key)) continue;
      seenEdge.add(key);
      if (protectedSet.has(key)) {
        addEdge(from, to, INF); // uncuttable, not recorded as a real edge
      } else {
        realEdges.push({ from, to });
        addEdge(from, to, 1);
      }
    }
  }

  // If the sink is reachable from the source through INF-capacity edges alone, every cut
  // must include a protected edge — there is no valid cut. Check while caps are pristine.
  const visited = new Array(total).fill(false);
  {
    const queue = [source];
    let head = 0;
    visited[source] = true;
    while (head < queue.length) {
      const node = queue[head++];
      for (const e of adjacency[node]) {
        const next = edgeTo[e];
        if (edgeCap[e] >= INF && !visited[next]) {
          visited[next] = true;
          queue.push(next);
        }
      }
    }
  }
  if (visited[sink]) {
    return { cutEdges: [], flow: 0, hasUncuttableSink: false, blockedByProtected: true };
  }

  // --- Dinic max-flow ---
  const level = new Array(total).fill(-1);
  const iterPtr = new Array(total).fill(0);

  const buildLevels = () => {
    level.fill(-1);
    const queue = [source];
    let head = 0;
    level[source] = 0;
    while (head < queue.length) {
      const node = queue[head++];
      for (const e of adjacency[node]) {
        const next = edgeTo[e];
        if (edgeCap[e] > 0 && level[next] < 0) {
          level[next] = level[node] + 1;
          queue.push(next);
        }
      }
    }
    return level[sink] >= 0;
  };

  const nextAdmissibleEdge = (node) => {
    while (iterPtr[node] < adjacency[node].length) {
      const e = adjacency[node][iterPtr[node]];
      const next = edgeTo[e];
      if (edgeCap[e] > 0 && level[next] === level[node] + 1) return e;
      iterPtr[node] += 1;
    }
    return -1;
  };

  const pushBlockingFlow = () => {
    const nodes = [source]; // current DFS path
    const edges = []; // edges[i] = edge taken from nodes[i] to nodes[i+1]
    while (nodes.length) {
      const node = nodes[nodes.length - 1];
      if (node === sink) {
        // Augment by the path bottleneck, then retreat to just before the first edge
        // that saturated (earlier edges keep residual and can carry more flow).
        let bottleneck = Infinity;
        for (const e of edges) if (edgeCap[e] < bottleneck) bottleneck = edgeCap[e];
        let firstSaturated = edges.length;
        for (let i = 0; i < edges.length; i++) {
          const e = edges[i];
          edgeCap[e] -= bottleneck;
          edgeCap[e ^ 1] += bottleneck;
          if (firstSaturated === edges.length && edgeCap[e] === 0) firstSaturated = i;
        }
        nodes.length = firstSaturated + 1;
        edges.length = firstSaturated;
        continue;
      }
      const e = nextAdmissibleEdge(node);
      if (e !== -1) {
        nodes.push(edgeTo[e]);
        edges.push(e);
      } else {
        // Dead end: drop it from the level graph, back up, step the parent past this edge.
        level[node] = -1;
        nodes.pop();
        if (edges.length) {
          const back = edges.pop();
          iterPtr[edgeTo[back ^ 1]] += 1;
        }
      }
    }
  };

  while (buildLevels()) {
    iterPtr.fill(0);
    pushBlockingFlow();
  }
  const flow = edgeCap[targetSinkFwd ^ 1]; // backward residual on target->sink = flow pushed

  // --- extract the cut nearest the sink ---
  // T = nodes that can still reach the sink in the residual graph (reverse BFS). Real
  // edges crossing from the source side (∉T) into T are the last-hop imports, and the
  // min-cut theorem guarantees they are saturated.
  const canReach = new Array(total).fill(false);
  {
    const queue = [sink];
    let head = 0;
    canReach[sink] = true;
    while (head < queue.length) {
      const node = queue[head++];
      for (const e of adjacency[node]) {
        const neighbor = edgeTo[e];
        if (edgeCap[e ^ 1] > 0 && !canReach[neighbor]) {
          canReach[neighbor] = true;
          queue.push(neighbor);
        }
      }
    }
  }
  const seen = new Set();
  const cutEdges = [];
  for (const { from, to } of realEdges) {
    if (!canReach[from] && canReach[to]) {
      const key = from * n + to;
      if (!seen.has(key)) {
        seen.add(key);
        cutEdges.push({ from, to });
      }
    }
  }
  cutEdges.sort((a, b) => a.from - b.from || a.to - b.to);

  // Built-in sanity check: the number of saturated last-hop edges must equal the flow.
  if (cutEdges.length !== flow) {
    throw new Error(`min-cut invariant violated: flow ${flow} != |cut| ${cutEdges.length}`);
  }
  return { cutEdges, flow, hasUncuttableSink: false, blockedByProtected: false };
}
