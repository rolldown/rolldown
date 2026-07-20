// Deterministic diff of two module-graph sidecars: what a change did to the INITIAL
// LOAD (the eager set), before paying a throttled measure. Alignment is by module id
// (indices shift between builds), ids slash-normalized. The headline is the eager-tier
// diff — modules that ENTERED or LEFT the initial load — computed with the same overlay
// `eagerSet` the rest of the kit uses, so it agrees with what-if's predictions.
//
// Attribution is exclusion-aware (unigraph's TwinGraph "+1 not +8" rule): a new module
// that reuses 7 pre-existing deps attributes 1 changed module, not 8 — because the set
// difference only counts nodes that themselves entered/left. Grouping folds changed
// modules under their nearest changed idom-ancestor and reports untouched intermediates
// as a skip count instead of listing them (token-cheap for agents).

import { eagerSet } from './module-graph.mjs';

const norm = (id) => id.replaceAll('\\', '/');
const byBytesDesc = (a, b) => b.bytes - a.bytes || a.id.localeCompare(b.id);

/** Eager module ids (slash-normalized) for a loaded graph. */
function eagerIds(graph) {
  const ids = new Set();
  for (const i of eagerSet(graph)) ids.add(norm(graph.modules[i].id));
  return ids;
}

// Static import target ids of a module (dynamic edges don't shape the eager structure).
function staticImportIds(graph, mod) {
  const ids = new Set();
  for (const [to, isDynamic] of mod.imports ?? []) {
    if (!isDynamic) ids.add(norm(graph.modules[to].id));
  }
  return ids;
}

// Fold changed modules under their nearest changed idom-ancestor. Each returned group is
// keyed by the TOPMOST changed ancestor on the chain; `skipped` counts the unchanged
// intermediate modules between members and that root (the ones we don't list).
function groupByChangedAncestor(graph, changedList, changedIdSet) {
  const idToIndex = new Map(graph.modules.map((m, i) => [norm(m.id), i]));
  const resolve = (startId) => {
    const idx = idToIndex.get(startId);
    let cur = idx == null ? null : graph.modules[idx].idom;
    let rootId = startId; // own root until a changed ancestor is found
    const pending = [];
    const skipped = new Set();
    while (cur != null) {
      const id = norm(graph.modules[cur].id);
      if (changedIdSet.has(id)) {
        rootId = id;
        for (const p of pending) skipped.add(p);
        pending.length = 0;
      } else {
        pending.push(id);
      }
      cur = graph.modules[cur].idom;
    }
    return { rootId, skipped };
  };
  const groups = new Map();
  for (const item of changedList) {
    const { rootId, skipped } = resolve(item.id);
    let g = groups.get(rootId);
    if (!g) { g = { rootId, bytes: 0, count: 0, skipped: new Set() }; groups.set(rootId, g); }
    g.bytes += item.bytes;
    g.count += 1;
    for (const s of skipped) g.skipped.add(s);
  }
  return [...groups.values()]
    .map((g) => ({ rootId: g.rootId, bytes: g.bytes, count: g.count, skipped: g.skipped.size }))
    .sort((a, b) => b.bytes - a.bytes || a.rootId.localeCompare(b.rootId));
}

/**
 * Diff `before` (e.g. the pinned baseline sidecar) against `after` (the current build).
 * Returns node classification (added / removed / bytesChanged / edgesChanged) and the
 * eager-tier diff (entered / left with byte totals) plus exclusion-aware groupings.
 * Ids are identity — a rename reads as one add + one remove (no rename detection).
 */
export function diffGraphs(before, after) {
  const beforeById = new Map(before.modules.map((m) => [norm(m.id), m]));
  const afterById = new Map(after.modules.map((m) => [norm(m.id), m]));

  const added = [];
  const removed = [];
  const bytesChanged = [];
  const edgesChanged = [];
  for (const [id, m] of afterById) {
    if (!beforeById.has(id)) added.push({ id, bytes: m.bytes });
  }
  for (const [id, m] of beforeById) {
    const am = afterById.get(id);
    if (!am) { removed.push({ id, bytes: m.bytes }); continue; }
    if (am.bytes !== m.bytes) {
      bytesChanged.push({ id, before: m.bytes, after: am.bytes, delta: am.bytes - m.bytes });
    }
    const bImp = staticImportIds(before, m);
    const aImp = staticImportIds(after, am);
    if (bImp.size !== aImp.size || [...aImp].some((t) => !bImp.has(t))) {
      edgesChanged.push({ id });
    }
  }

  const beforeEager = eagerIds(before);
  const afterEager = eagerIds(after);
  const entered = [];
  const left = [];
  for (const id of afterEager) if (!beforeEager.has(id)) entered.push({ id, bytes: afterById.get(id).bytes });
  for (const id of beforeEager) if (!afterEager.has(id)) left.push({ id, bytes: beforeById.get(id).bytes });
  entered.sort(byBytesDesc);
  left.sort(byBytesDesc);
  const enteredBytes = entered.reduce((s, m) => s + m.bytes, 0);
  const leftBytes = left.reduce((s, m) => s + m.bytes, 0);

  return {
    added: added.sort(byBytesDesc),
    removed: removed.sort(byBytesDesc),
    bytesChanged: bytesChanged.sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta) || a.id.localeCompare(b.id)),
    edgesChanged: edgesChanged.sort((a, b) => a.id.localeCompare(b.id)),
    entered,
    left,
    enteredBytes,
    leftBytes,
    enteredGroups: groupByChangedAncestor(after, entered, new Set(entered.map((m) => m.id))),
    leftGroups: groupByChangedAncestor(before, left, new Set(left.map((m) => m.id))),
    changed: entered.length + left.length + added.length + removed.length !== 0,
  };
}
