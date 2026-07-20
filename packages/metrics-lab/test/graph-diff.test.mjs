// P2 — baseline graph snapshot diff. Aligns two sidecars by id and reports what changed
// about the INITIAL LOAD (the eager set). Exclusion-aware attribution (+1 not +8) and the
// graph-diff <-> what-if agreement (the cross-check the E2E rests on) are the key asserts.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { diffGraphs } from '../lib/graph-diff.mjs';
import { whatIf } from '../lib/module-graph.mjs';
import { indexOf, makeGraph } from './graph-fixtures.mjs';

test('+1 not +8: a new module reusing existing deps attributes only itself', () => {
  // before: entry -> core -> d1..d3 (all eager). after: entry also -> T -> core (reuse).
  const before = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      {
        id: 'core',
        bytes: 100,
        imports: [
          [2, false],
          [3, false],
          [4, false],
        ],
      },
      { id: 'd1', bytes: 10, imports: [] },
      { id: 'd2', bytes: 10, imports: [] },
      { id: 'd3', bytes: 10, imports: [] },
    ],
    ['entry'],
  );
  const after = makeGraph(
    [
      {
        id: 'entry',
        bytes: 5,
        imports: [
          [1, false],
          [5, false],
        ],
      },
      {
        id: 'core',
        bytes: 100,
        imports: [
          [2, false],
          [3, false],
          [4, false],
        ],
      },
      { id: 'd1', bytes: 10, imports: [] },
      { id: 'd2', bytes: 10, imports: [] },
      { id: 'd3', bytes: 10, imports: [] },
      { id: 'T', bytes: 42, imports: [[1, false]] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(before, after);
  // only T entered the eager set — core + d1..d3 were already eager (the +1, not +5)
  assert.deepEqual(
    diff.entered.map((m) => m.id),
    ['T'],
  );
  assert.equal(diff.enteredBytes, 42);
  assert.deepEqual(
    diff.added.map((m) => m.id),
    ['T'],
  );
  assert.equal(diff.left.length, 0);
});

test('module leaving eager via a defer; graph-diff agrees with what-if', () => {
  // before: entry -> charts -> dep (both eager, dep only via charts).
  const before = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'charts', bytes: 1000, imports: [[2, false]] },
      { id: 'dep', bytes: 300, imports: [] },
    ],
    ['entry'],
  );
  // after: the import became dynamic — charts (and dep) leave the initial load.
  const after = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, true]] },
      { id: 'charts', bytes: 1000, imports: [[2, false]] },
      { id: 'dep', bytes: 300, imports: [] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(before, after);
  assert.deepEqual(diff.left.map((m) => m.id).sort(), ['charts', 'dep']);
  assert.equal(diff.leftBytes, 1300);
  assert.equal(diff.entered.length, 0);
  // THE cross-check: graph-diff's LEFT set == what-if's predicted removed set on `before`.
  const predicted = whatIf(before, indexOf(before, 'charts'));
  assert.deepEqual(diff.left.map((m) => m.id).sort(), predicted.removed.map((m) => m.id).sort());
  assert.equal(diff.leftBytes, predicted.removedBytes);
});

test('edge retarget: leaves detach, enter, and the edge is flagged', () => {
  // before: entry -> a -> x (x only via a); y exists but orphan (not eager).
  const before = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'a', bytes: 10, imports: [[2, false]] },
      { id: 'x', bytes: 500, imports: [] },
      { id: 'y', bytes: 400, imports: [] },
    ],
    ['entry'],
  );
  // after: a retargets x -> y. x becomes unreachable; y becomes eager.
  const after = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'a', bytes: 10, imports: [[3, false]] },
      { id: 'x', bytes: 500, imports: [] },
      { id: 'y', bytes: 400, imports: [] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(before, after);
  assert.deepEqual(
    diff.entered.map((m) => m.id),
    ['y'],
  );
  assert.deepEqual(
    diff.left.map((m) => m.id),
    ['x'],
  );
  assert.deepEqual(
    diff.edgesChanged.map((m) => m.id),
    ['a'],
  );
  assert.equal(diff.added.length, 0);
  assert.equal(diff.removed.length, 0);
});

test('renamed id reads as one add + one remove (no rename detection)', () => {
  const before = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'src/old.ts', bytes: 200, imports: [] },
    ],
    ['entry'],
  );
  const after = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'src/new.ts', bytes: 200, imports: [] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(before, after);
  assert.deepEqual(
    diff.added.map((m) => m.id),
    ['src/new.ts'],
  );
  assert.deepEqual(
    diff.removed.map((m) => m.id),
    ['src/old.ts'],
  );
  assert.deepEqual(
    diff.entered.map((m) => m.id),
    ['src/new.ts'],
  );
  assert.deepEqual(
    diff.left.map((m) => m.id),
    ['src/old.ts'],
  );
});

test('bytes-changed is detected when a module grows', () => {
  const before = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'm', bytes: 100, imports: [] },
    ],
    ['entry'],
  );
  const after = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'm', bytes: 180, imports: [] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(before, after);
  assert.deepEqual(diff.bytesChanged, [{ id: 'm', before: 100, after: 180, delta: 80 }]);
  assert.equal(diff.entered.length, 0);
  assert.equal(diff.left.length, 0);
});

test('exclusion-aware grouping folds entered modules under a changed root, skipping unchanged intermediates', () => {
  // before: entry -> u (u eager, imports nothing).
  const before = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'u', bytes: 50, imports: [] },
    ],
    ['entry'],
  );
  // after: entry -> T -> u -> nd. T and nd are new-eager; u is an UNCHANGED intermediate
  // on the dominator chain nd -> u -> T (u stays eager, same bytes).
  const after = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'T', bytes: 30, imports: [[2, false]] },
      { id: 'u', bytes: 50, imports: [[3, false]] },
      { id: 'nd', bytes: 70, imports: [] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(before, after);
  assert.deepEqual(diff.entered.map((m) => m.id).sort(), ['T', 'nd']);
  // one group rooted at T, holding both changed modules, with u reported as skipped
  assert.equal(diff.enteredGroups.length, 1);
  const g = diff.enteredGroups[0];
  assert.equal(g.rootId, 'T');
  assert.equal(g.count, 2);
  assert.equal(g.bytes, 100); // T(30) + nd(70)
  assert.equal(g.skipped, 1); // u
});

test('identical graphs diff to nothing', () => {
  const g = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'a', bytes: 10, imports: [] },
    ],
    ['entry'],
  );
  const diff = diffGraphs(g, g);
  assert.equal(diff.changed, false);
  assert.equal(diff.entered.length, 0);
  assert.equal(diff.left.length, 0);
  assert.equal(diff.added.length, 0);
  assert.equal(diff.removed.length, 0);
});
