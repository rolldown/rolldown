// P1a — edge-override overlay engine + what-if equivalence.
//
// Rule 7: the refactored `whatIf` must preserve observable behavior EXACTLY. The
// pre-refactor grow/prune implementation is kept here verbatim as `oldWhatIf` and
// used as the oracle. If any hand graph diverges, this test fails loudly (STOP).

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { deferAllInto, eagerSet, evalOverrides, whatIf } from '../lib/module-graph.mjs';
import { indexOf, makeGraph } from './graph-fixtures.mjs';

// --- oracle: the exact pre-refactor whatIf (grow-then-fixpoint-prune UR closure) -----
function oldWhatIf(graph, target, keep = []) {
  const keepSet = new Set(keep);
  const mods = graph.modules;
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

// --- hand graphs -----------------------------------------------------------------
const graphs = {
  // entry -> a -> c, entry -> b -> c: c joins under entry (idom = entry, not a/b).
  diamond: makeGraph(
    [
      {
        id: 'entry',
        bytes: 10,
        imports: [
          [1, false],
          [2, false],
        ],
      },
      { id: 'a', bytes: 100, imports: [[3, false]] },
      { id: 'b', bytes: 200, imports: [[3, false]] },
      { id: 'c', bytes: 1000, imports: [] },
    ],
    ['entry'],
  ),

  // entry -> route -> heavy -> util; entry -(dynamic)-> lazy -> lazydep
  chainDynamic: makeGraph(
    [
      {
        id: 'entry',
        bytes: 5,
        imports: [
          [1, false],
          [4, true],
        ],
      },
      { id: 'route', bytes: 10, imports: [[2, false]] },
      { id: 'heavy', bytes: 5000, imports: [[3, false]] },
      { id: 'util', bytes: 300, imports: [] },
      { id: 'lazy', bytes: 70, imports: [[5, false]] },
      { id: 'lazydep', bytes: 30, imports: [] },
    ],
    ['entry'],
  ),

  // two entries share a module -> shared hangs off the virtual root (idom null).
  multiEntry: makeGraph(
    [
      { id: 'a', bytes: 10, imports: [[2, false]] },
      { id: 'b', bytes: 20, imports: [[2, false]] },
      { id: 'shared', bytes: 500, imports: [] },
    ],
    ['a', 'b'],
  ),

  // entry -> a <-> b cycle inside a dominated region.
  cycle: makeGraph(
    [
      { id: 'entry', bytes: 1, imports: [[1, false]] },
      { id: 'a', bytes: 40, imports: [[2, false]] },
      { id: 'b', bytes: 60, imports: [[1, false]] },
    ],
    ['entry'],
  ),

  // Shared-internals: entry imports fa and fb; both pull a shared `core` (+ its dep),
  // dominated by neither individually (idom = entry). Deferring one frees only its
  // own slice; deferring BOTH frees core too — combined > sum.
  sharedInternals: makeGraph(
    [
      {
        id: 'entry',
        bytes: 10,
        imports: [
          [1, false],
          [2, false],
        ],
      },
      { id: 'fa', bytes: 100, imports: [[3, false]] },
      { id: 'fb', bytes: 100, imports: [[3, false]] },
      { id: 'core', bytes: 800, imports: [[4, false]] },
      { id: 'coredep', bytes: 200, imports: [] },
    ],
    ['entry'],
  ),

  // Sentry reachability: entry -> target -> sentry -> sdep; target -> d. Deferring
  // target with --keep sentry must keep sentry + sdep eager even though they are only
  // reachable via target (the extraRoots re-seed).
  sentry: makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      {
        id: 'target',
        bytes: 50,
        imports: [
          [2, false],
          [4, false],
        ],
      },
      { id: 'sentry', bytes: 300, imports: [[3, false]] },
      { id: 'sdep', bytes: 70, imports: [] },
      { id: 'd', bytes: 25, imports: [] },
    ],
    ['entry'],
  ),
};

// Observable behavior = exactly what cmdWhatIf consumes: for an unreachable target
// only the flags matter (the removed set is never displayed); otherwise the byte/
// count totals, the removed ids, and the cut-edge ids.
function observable(result) {
  if (result.notStaticallyReachable) {
    return { notStaticallyReachable: true, alreadyLazy: result.alreadyLazy };
  }
  return {
    notStaticallyReachable: false,
    alreadyLazy: result.alreadyLazy,
    removedBytes: result.removedBytes,
    removedCount: result.removedCount,
    removedIds: result.removed.map((m) => m.id),
    cutEdges: result.cutEdges,
  };
}

// --- anchor: the dominator builder reproduces the Rust producer's known values ------
test('makeGraph dominators match the Rust graph.rs fixtures', () => {
  const d = graphs.diamond;
  assert.equal(d.modules[indexOf(d, 'a')].retainedBytes, 100);
  assert.equal(d.modules[indexOf(d, 'b')].retainedBytes, 200);
  assert.equal(d.modules[indexOf(d, 'c')].idom, indexOf(d, 'entry'));
  assert.equal(d.modules[indexOf(d, 'entry')].retainedBytes, 1310);
  assert.equal(d.modules[indexOf(d, 'entry')].idom, null);

  const c = graphs.chainDynamic;
  assert.equal(c.modules[indexOf(c, 'route')].retainedBytes, 5310);
  assert.equal(c.modules[indexOf(c, 'route')].retainedModuleCount, 3);
  assert.equal(c.modules[indexOf(c, 'lazy')].dynamicOnly, true);
  assert.equal(c.modules[indexOf(c, 'lazydep')].dynamicOnly, true);
  assert.equal(c.modules[indexOf(c, 'lazy')].retainedBytes, 0);

  const m = graphs.multiEntry;
  assert.equal(m.modules[indexOf(m, 'shared')].idom, null);
  assert.equal(m.modules[indexOf(m, 'a')].retainedBytes, 10);
  assert.equal(m.modules[indexOf(m, 'b')].retainedBytes, 20);

  const cy = graphs.cycle;
  assert.equal(cy.modules[indexOf(cy, 'a')].retainedBytes, 100);
  assert.equal(cy.modules[indexOf(cy, 'b')].retainedBytes, 60);
});

// --- base eager set equals the sidecar's staticReachable set (cross-check) -----------
test('base eagerSet equals the staticReachable module set', () => {
  for (const [name, graph] of Object.entries(graphs)) {
    const base = eagerSet(graph);
    const fromFlags = new Set(graph.modules.flatMap((m, i) => (m.staticReachable ? [i] : [])));
    assert.deepEqual(
      [...base].sort((x, y) => x - y),
      [...fromFlags].sort((x, y) => x - y),
      `base eager mismatch on ${name}`,
    );
  }
});

// --- equivalence: new whatIf == old whatIf, every NON-ENTRY target x every keep combo.
// Entry targets are excluded by design since the 2026-07-19 consolidation: the old
// closure answered them with the entry's whole subtree (deletion, not deferral); the
// consolidated whatIf reports `isEntry` instead (asserted in its own test below).
test('whatIf overlay reproduces the old grow/prune closure exactly', () => {
  for (const [name, graph] of Object.entries(graphs)) {
    const ids = graph.modules.map((m) => m.id);
    const entrySet = new Set(graph.entryModules);
    // keep combinations: none, each single, and a couple of pairs
    const keepCombos = [[]];
    for (const k of ids) keepCombos.push([k]);
    for (let i = 0; i < ids.length; i++) {
      for (let j = i + 1; j < ids.length; j++) keepCombos.push([ids[i], ids[j]]);
    }
    for (let t = 0; t < graph.modules.length; t++) {
      if (entrySet.has(ids[t])) continue; // consolidated divergence, see above
      for (const keepIds of keepCombos) {
        const keep = keepIds.map((k) => indexOf(graph, k));
        if (keep.includes(t)) continue; // deferring X while keeping X is degenerate
        const got = observable(whatIf(graph, t, keep));
        const want = observable(oldWhatIf(graph, t, keep));
        assert.deepEqual(
          got,
          want,
          `divergence on ${name}: target=${ids[t]} keep=[${keepIds.join(',')}]`,
        );
      }
    }
  }
});

// --- consolidated entry answer: isEntry, nothing removable, no cut edges ------------
// An entry has no eager in-edge to cut — it IS the initial load. The pre-consolidation
// closure claimed the whole subtree (1310 bytes here) was "removable" with ZERO cut
// edges; that described deleting the entry and is documented via the oracle only.
test('entry target: consolidated whatIf reports isEntry with an empty result', () => {
  const g = graphs.diamond;
  const entry = indexOf(g, 'entry');
  // The pure overlay agrees: deferring the entry's in-edges (there are none) frees nothing.
  assert.equal(evalOverrides(g, deferAllInto(g, entry)).removedBytes, 0);
  const r = whatIf(g, entry, []);
  assert.equal(r.isEntry, true);
  assert.equal(r.removedBytes, 0);
  assert.equal(r.removedCount, 0);
  assert.deepEqual(r.cutEdges, []);
  assert.deepEqual(r.removed, []);
  // Non-entry results carry isEntry: false.
  assert.equal(whatIf(g, indexOf(g, 'c'), []).isEntry, false);
  // What the old answer was (the whole subtree) — kept as documentation of the change.
  assert.equal(oldWhatIf(g, entry, []).removedBytes, 1310);
});

// --- theorem: single-target overlay removedBytes == retainedBytes -------------------
// The dominator subtree a single-module cut frees is EXACTLY retainedBytes. Overlay
// (forward reachability) and the dominator fold agreeing on every non-entry module
// cross-checks both against each other and against the Rust sidecar's semantics.
test('single-target overlay equals retainedBytes/retainedModuleCount', () => {
  for (const [name, graph] of Object.entries(graphs)) {
    const entrySet = new Set(graph.entryModules);
    for (let t = 0; t < graph.modules.length; t++) {
      const mod = graph.modules[t];
      if (!mod.staticReachable || entrySet.has(mod.id)) continue;
      const { removedBytes, removedCount } = evalOverrides(graph, deferAllInto(graph, t));
      assert.equal(removedBytes, mod.retainedBytes, `retainedBytes mismatch on ${name}: ${mod.id}`);
      assert.equal(
        removedCount,
        mod.retainedModuleCount,
        `retainedModuleCount mismatch on ${name}: ${mod.id}`,
      );
    }
  }
});

// --- group pricing: combined > sum when internals are shared ------------------------
test('combined deferral exceeds the sum when internals are shared', () => {
  const g = graphs.sharedInternals;
  const fa = indexOf(g, 'fa');
  const fb = indexOf(g, 'fb');
  const retFa = g.modules[fa].retainedBytes; // 100 (own only; core dominated by entry)
  const retFb = g.modules[fb].retainedBytes; // 100
  assert.equal(retFa, 100);
  assert.equal(retFb, 100);
  const overrides = [...deferAllInto(g, fa), ...deferAllInto(g, fb)];
  const combined = evalOverrides(g, overrides);
  // deferring BOTH frees fa + fb + core + coredep = 100+100+800+200 = 1200
  assert.equal(combined.removedBytes, 1200);
  assert.ok(
    combined.removedBytes > retFa + retFb,
    `combined ${combined.removedBytes} should exceed summed ${retFa + retFb}`,
  );
  // shared internals = combined - sum = 1200 - 200 = 1000 (core + coredep)
  assert.equal(combined.removedBytes - (retFa + retFb), 1000);
});

// --- evalOverrides with no overrides removes nothing --------------------------------
test('evalOverrides([]) removes nothing and returns the base eager bytes', () => {
  const g = graphs.diamond;
  const r = evalOverrides(g, []);
  assert.equal(r.removedCount, 0);
  assert.equal(r.removedBytes, 0);
  const baseBytes = [...eagerSet(g)].reduce((s, i) => s + g.modules[i].bytes, 0);
  assert.equal(r.eagerBytes, baseBytes);
});

// --- 'remove' kind behaves like 'defer' for eager reachability ----------------------
test("override kind 'remove' matches 'defer' on eager reachability", () => {
  const g = graphs.diamond;
  const c = indexOf(g, 'c');
  const preds = g.staticPreds[c];
  const asDefer = evalOverrides(
    g,
    preds.map((p) => ({ from: p, to: c, kind: 'defer' })),
  );
  const asRemove = evalOverrides(
    g,
    preds.map((p) => ({ from: p, to: c, kind: 'remove' })),
  );
  assert.deepEqual(
    asRemove.removed.map((m) => m.id),
    asDefer.removed.map((m) => m.id),
  );
  assert.equal(asRemove.removedBytes, asDefer.removedBytes);
});
