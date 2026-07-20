// Reads rolldown's `module-graph.json` (emitted by devtools metrics mode) and answers
// deferral questions on it. The single engine is a read-only edge-override overlay
// (unigraph's EdgeOverrides): the initial load is forward reachability from the entries
// over static edges, and every "what leaves if ...?" recomputes it with the affected
// edges skipped. With no sentries a single module's closure equals its dominator
// subtree, so `removedBytes` matches the report's `retainedBytes`.

import fs from 'node:fs';
import path from 'node:path';

/** Candidate report dirs, most specific first. Each is checked for module-graph.json. */
export function moduleGraphCandidates({ reportDir, demoMetricsDir, dist }) {
  const candidates = [];
  if (reportDir) {
    candidates.push(
      reportDir.endsWith('.json') ? reportDir : path.join(reportDir, 'module-graph.json'),
    );
  }
  if (demoMetricsDir) candidates.push(path.join(demoMetricsDir, 'module-graph.json'));
  if (dist) {
    // The report lands relative to the BUILD's cwd, which may sit one or more
    // levels above the dist dir (nested outDirs like build/dist, monorepo app
    // dirs) - walk up a few levels rather than assuming dist's direct parent.
    let dir = path.dirname(dist);
    for (let i = 0; i < 3; i++) {
      candidates.push(
        path.join(dir, '.rolldown', 'metrics', 'module-graph.json'),
        path.join(dir, 'node_modules', '.rolldown', 'metrics', 'module-graph.json'),
      );
      const parent = path.dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
  }
  return candidates;
}

// Static predecessor lists (only importers that are themselves part of the initial
// load can pin a module into it). Factored out so hand-built test graphs get the
// exact same predecessor construction the loader uses.
export function computeStaticPreds(modules) {
  const staticPreds = modules.map(() => []);
  modules.forEach((mod, from) => {
    if (!mod.staticReachable) return;
    for (const [to, isDynamic] of mod.imports ?? []) {
      if (!isDynamic) staticPreds[to].push(from);
    }
  });
  return staticPreds;
}

export function loadModuleGraph(candidates) {
  const file = candidates.find((candidate) => fs.existsSync(candidate));
  if (!file) return null;
  const data = JSON.parse(fs.readFileSync(file, 'utf8'));
  return { file, ...data, staticPreds: computeStaticPreds(data.modules) };
}

/**
 * Find a module by query: exact id, then unique suffix, then unique substring
 * (case-insensitive). Returns `{ index }`, `{ ambiguous: [...] }`, or `null`.
 */
export function resolveModule(graph, query) {
  const q = query.replaceAll('\\', '/');
  const ids = graph.modules.map((m) => m.id);
  const exact = ids.findIndex((id) => id === q);
  if (exact >= 0) return { index: exact };
  const lower = q.toLowerCase();
  for (const match of [
    (id) => id.toLowerCase().endsWith(lower),
    (id) => id.toLowerCase().includes(lower),
  ]) {
    const hits = [];
    ids.forEach((id, i) => {
      if (match(id)) hits.push(i);
    });
    if (hits.length === 1) return { index: hits[0] };
    if (hits.length > 1) {
      const rows = hits
        .map((i) => graph.modules[i])
        .sort((a, b) => b.retainedBytes - a.retainedBytes || a.id.localeCompare(b.id))
        .slice(0, 8)
        .map((m) => m.id);
      return { ambiguous: rows };
    }
  }
  return null;
}

// --- edge-override overlay engine ------------------------------------------------
// The initial load = everything reachable from the entries over STATIC edges. Every
// deferral question ("what leaves if these import edges become dynamic / vanish?") is
// answered by recomputing that forward reachability with the affected edges skipped —
// no graph mutation. This is the read-only overlay unigraph's EdgeOverrides provides,
// and the substrate `cut` and `graph-diff` price their answers with.

/** Indices of the entry modules (cached on the graph). Ids not present are skipped. */
function entryIndices(graph) {
  if (graph.__entryIdx) return graph.__entryIdx;
  const idOf = new Map(graph.modules.map((m, i) => [m.id, i]));
  const idx = (graph.entryModules ?? []).map((id) => idOf.get(id)).filter((i) => i != null);
  graph.__entryIdx = idx;
  return idx;
}

/**
 * The eager set: module indices reachable from the roots over static edges, with the
 * `overrides` edges skipped. `overrides` is an array of `{ from, to, kind }` (both
 * 'defer' and 'remove' stop the edge pinning `to` eager). `extraRoots` seeds extra
 * always-eager roots (used to model `--keep` sentries: a kept module and everything
 * only it needs stay eager even after the cut). The bare call (entries only, no
 * overrides) is the base eager set and is cached on the graph object — do not mutate
 * the returned Set.
 */
export function eagerSet(graph, overrides = [], extraRoots = null) {
  const n = graph.modules.length;
  const bare = overrides.length === 0 && extraRoots == null;
  if (bare && graph.__baseEager) return graph.__baseEager;
  // Key edges as from*n + to in a Set — no string allocation on 50k-module graphs.
  const blocked = overrides.length ? new Set(overrides.map((o) => o.from * n + o.to)) : null;
  const roots = extraRoots == null ? entryIndices(graph) : [...entryIndices(graph), ...extraRoots];
  const seen = new Set();
  const stack = [];
  for (const r of roots) {
    if (r != null && !seen.has(r)) {
      seen.add(r);
      stack.push(r);
    }
  }
  while (stack.length > 0) {
    const v = stack.pop();
    for (const [to, isDynamic] of graph.modules[v].imports ?? []) {
      if (isDynamic) continue;
      if (blocked && blocked.has(v * n + to)) continue;
      if (!seen.has(to)) {
        seen.add(to);
        stack.push(to);
      }
    }
  }
  if (bare) graph.__baseEager = seen;
  return seen;
}

/**
 * Price a batch of edge overrides against the base (current) eager set. Returns the
 * resulting eager set and its bytes, plus what LEFT the initial load: `removed` is the
 * module objects sorted `bytes desc, id asc` (matching the display order), with total
 * bytes and count. `extraRoots` models `--keep` sentries (see `eagerSet`).
 */
export function evalOverrides(graph, overrides, extraRoots = null) {
  const mods = graph.modules;
  const base = eagerSet(graph);
  const next = eagerSet(graph, overrides, extraRoots);
  let eagerBytes = 0;
  for (const v of next) eagerBytes += mods[v].bytes;
  const removed = [];
  for (const v of base) if (!next.has(v)) removed.push(mods[v]);
  removed.sort((a, b) => b.bytes - a.bytes || a.id.localeCompare(b.id));
  const removedBytes = removed.reduce((sum, m) => sum + m.bytes, 0);
  return { eager: next, eagerBytes, removed, removedBytes, removedCount: removed.length };
}

/**
 * Module-level shorthand: override every static import edge into `target` to 'defer'.
 * These are exactly the edges the what-if module query cuts. (`keep` is accepted for a
 * symmetric signature but is applied by the caller as `extraRoots`, not here — the cut
 * itself is the same set of edges regardless of sentries.)
 */
export function deferAllInto(graph, target) {
  return graph.staticPreds[target].map((p) => ({ from: p, to: target, kind: 'defer' }));
}

/**
 * The what-if-deferred closure: every module that would leave the initial load if all
 * static import edges into `target` became dynamic. `keep` modules are sentries — they
 * (and everything only they need) stay eager. Thin wrapper over the overlay: deferring
 * `target`'s in-edges and recomputing forward reachability (with sentries re-seeded as
 * roots) IS the unique-reachable set.
 *
 * An entry target returns `isEntry: true` with an empty result: an entry has no eager
 * in-edge to cut — it IS the initial load. (Consolidated 2026-07-19: the pre-overlay
 * grow/prune closure answered this case with the entry's whole forward subtree and
 * zero cut edges, which read as "removable" but described deleting the entry, not
 * deferring it. The old implementation survives only as the test oracle.)
 */
export function whatIf(graph, target, keep = []) {
  const mods = graph.modules;
  const base = {
    target: mods[target],
    removed: [],
    removedBytes: 0,
    removedCount: 0,
    cutEdges: [],
    alreadyLazy: mods[target].dynamicOnly === true,
    notStaticallyReachable: mods[target].staticReachable === false,
    isEntry: entryIndices(graph).includes(target),
  };
  // Targets with no eager in-edge to cut: entries (they are roots), and modules that
  // cost the initial load nothing (dynamic-only or unreachable). The caller reports the
  // flag and never shows a removed set.
  if (base.isEntry || base.notStaticallyReachable) return base;
  // Sentries stay eager via extraRoots; the target itself is never its own sentry.
  const extraRoots = keep.filter((k) => k !== target);
  const {
    eager: next,
    removed,
    removedBytes,
    removedCount,
  } = evalOverrides(graph, deferAllInto(graph, target), extraRoots.length ? extraRoots : null);
  // Cut edges are the target's static importers that STAY eager (all static preds are
  // themselves eager, so "survives" = "still in the next eager set"); an importer that
  // is itself removed needs no edit — it is gone from the initial load anyway.
  const cutEdges = graph.staticPreds[target]
    .filter((p) => next.has(p))
    .map((p) => mods[p].id)
    .sort();
  return { ...base, removed, removedBytes, removedCount, cutEdges };
}
