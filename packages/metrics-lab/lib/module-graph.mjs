// Reads rolldown's `module-graph.json` (emitted by devtools metrics mode) and answers
// "what if this module's import edge were deferred" queries: the unique-reachable
// closure with optional sentry modules — SimplifyGraph's UR algorithm transplanted
// from CFGs onto the static module graph. With no sentries the closure equals the
// module's dominator subtree, so `removedBytes` matches the report's `retainedBytes`.

import fs from 'node:fs';
import path from 'node:path';

/** Candidate report dirs, most specific first. Each is checked for module-graph.json. */
export function moduleGraphCandidates({ reportDir, demoMetricsDir, dist }) {
  const candidates = [];
  if (reportDir) {
    candidates.push(reportDir.endsWith('.json') ? reportDir : path.join(reportDir, 'module-graph.json'));
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

export function loadModuleGraph(candidates) {
  const file = candidates.find((candidate) => fs.existsSync(candidate));
  if (!file) return null;
  const data = JSON.parse(fs.readFileSync(file, 'utf8'));
  // Static predecessor lists (only importers that are themselves part of the initial
  // load can pin a module into it).
  const staticPreds = data.modules.map(() => []);
  data.modules.forEach((mod, from) => {
    if (!mod.staticReachable) return;
    for (const [to, isDynamic] of mod.imports ?? []) {
      if (!isDynamic) staticPreds[to].push(from);
    }
  });
  return { file, ...data, staticPreds };
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

/**
 * The what-if-deferred closure: every module that would leave the initial load if all
 * static import edges into `target` became dynamic. `keep` modules are sentries — they
 * (and everything only they need) stay eager, and traversal never enters them.
 */
export function whatIf(graph, target, keep = []) {
  const keepSet = new Set(keep);
  const mods = graph.modules;

  // Grow: everything statically reachable from the target (without crossing sentries).
  const inSet = new Set([target]);
  const stack = [target];
  while (stack.length > 0) {
    const v = stack.pop();
    for (const [to, isDynamic] of mods[v].imports ?? []) {
      if (!isDynamic && !inSet.has(to) && !keepSet.has(to)) {
        inSet.add(to);
        stack.push(to);
      }
    }
  }
  // Prune to the unique-reachable set: a module with a static importer outside the set
  // stays in the initial load through that other path, and its own imports may then be
  // pinned too — iterate to the fixpoint.
  let changed = true;
  while (changed) {
    changed = false;
    for (const v of [...inSet]) {
      if (v === target) continue;
      const pinned = graph.staticPreds[v].some((p) => !inSet.has(p) || keepSet.has(p));
      if (pinned) {
        inSet.delete(v);
        changed = true;
      }
    }
  }

  const removed = [...inSet]
    .map((i) => mods[i])
    .sort((a, b) => b.bytes - a.bytes || a.id.localeCompare(b.id));
  const removedBytes = removed.reduce((sum, m) => sum + m.bytes, 0);
  const cutEdges = graph.staticPreds[target]
    .filter((p) => !inSet.has(p))
    .map((p) => mods[p].id)
    .sort();
  return {
    target: mods[target],
    removed,
    removedBytes,
    removedCount: removed.length,
    cutEdges,
    alreadyLazy: mods[target].dynamicOnly === true,
    notStaticallyReachable: mods[target].staticReachable === false,
  };
}
