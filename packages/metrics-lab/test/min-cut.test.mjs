// P1b — minimum edge cut. Cases mirror unigraph's min_cut.rs: the join beats the
// leaves, parallel paths cut at the sink-nearest edges, protection routes the cut
// around, an all-protected path blocks, an entry sink is uncuttable, an orphan needs
// no cut, and a 50k-deep chain must not overflow (fully iterative solver).

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { minCut } from '../lib/min-cut.mjs';
import { deferAllInto, evalOverrides, whatIf } from '../lib/module-graph.mjs';
import { chain, indexOf, makeGraph } from './graph-fixtures.mjs';

const cutIds = (graph, result) =>
  result.cutEdges.map((e) => `${graph.modules[e.from].id}->${graph.modules[e.to].id}`).sort();

// entry -> gate -> {p1,p2} -> X : all paths to X run through gate.
const bottleneck = makeGraph(
  [
    { id: 'entry', bytes: 5, imports: [[1, false]] },
    {
      id: 'gate',
      bytes: 10,
      imports: [
        [2, false],
        [4, false],
      ],
    },
    { id: 'p1', bytes: 20, imports: [[3, false]] },
    { id: 'X', bytes: 1000, imports: [] },
    { id: 'p2', bytes: 20, imports: [[3, false]] },
  ],
  ['entry'],
);

// entry -> p1 -> X, entry -> p2 -> X : two independent paths, no shared bottleneck.
const parallel = makeGraph(
  [
    {
      id: 'entry',
      bytes: 5,
      imports: [
        [1, false],
        [3, false],
      ],
    },
    { id: 'p1', bytes: 20, imports: [[2, false]] },
    { id: 'X', bytes: 1000, imports: [] },
    { id: 'p2', bytes: 20, imports: [[2, false]] },
  ],
  ['entry'],
);

test('bottleneck: min cut is 1 at the join, not 2 at the leaves', () => {
  const X = indexOf(bottleneck, 'X');
  const result = minCut(bottleneck, X);
  assert.equal(result.flow, 1);
  assert.deepEqual(cutIds(bottleneck, result), ['entry->gate']);
  // naive what-if would touch both leaf edges into X
  assert.equal(whatIf(bottleneck, X).cutEdges.length, 2);
});

test('parallel paths: min cut is 2, taken at the sink-nearest edges', () => {
  const X = indexOf(parallel, 'X');
  const result = minCut(parallel, X);
  assert.equal(result.flow, 2);
  // sink-nearest: the last-hop edges into X, NOT the edges out of entry
  assert.deepEqual(cutIds(parallel, result), ['p1->X', 'p2->X']);
});

test('protecting a leaf edge forces the cut to route around it', () => {
  const X = indexOf(parallel, 'X');
  const p1 = indexOf(parallel, 'p1');
  const result = minCut(parallel, X, [{ from: p1, to: X }]);
  assert.equal(result.flow, 2);
  // p1->X is protected, so the p1 path is severed higher up (entry->p1)
  assert.deepEqual(cutIds(parallel, result), ['entry->p1', 'p2->X']);
});

test('protecting the bottleneck pushes the cut down to the leaves', () => {
  const X = indexOf(bottleneck, 'X');
  const entry = indexOf(bottleneck, 'entry');
  const gate = indexOf(bottleneck, 'gate');
  const result = minCut(bottleneck, X, [{ from: entry, to: gate }]);
  assert.equal(result.flow, 2);
  assert.deepEqual(cutIds(bottleneck, result), ['p1->X', 'p2->X']);
});

test('an all-protected path is blocked_by_protected (no valid cut)', () => {
  const g = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, false]] },
      { id: 'X', bytes: 100, imports: [] },
    ],
    ['entry'],
  );
  const result = minCut(g, indexOf(g, 'X'), [{ from: indexOf(g, 'entry'), to: indexOf(g, 'X') }]);
  assert.equal(result.blockedByProtected, true);
  assert.deepEqual(result.cutEdges, []);
});

test('an entry sink is uncuttable', () => {
  const result = minCut(parallel, indexOf(parallel, 'entry'));
  assert.equal(result.hasUncuttableSink, true);
  assert.deepEqual(result.cutEdges, []);
});

test('a dynamic-only / unreachable target needs no cut (flow 0)', () => {
  const g = makeGraph(
    [
      { id: 'entry', bytes: 5, imports: [[1, true]] }, // dynamic import only
      { id: 'X', bytes: 100, imports: [] },
    ],
    ['entry'],
  );
  const result = minCut(g, indexOf(g, 'X'));
  assert.equal(result.flow, 0);
  assert.deepEqual(result.cutEdges, []);
  assert.equal(result.blockedByProtected, false);
  assert.equal(result.hasUncuttableSink, false);
});

test('deep 50k-level chain: iterative solver returns the last hop, no overflow', () => {
  const depth = 50000;
  const g = chain(depth);
  const result = minCut(g, indexOf(g, `n${depth}`));
  assert.equal(result.flow, 1);
  assert.deepEqual(cutIds(g, result), [`n${depth - 1}->n${depth}`]);
});

test('flow == |cut| holds and the cut actually detaches X', () => {
  for (const g of [bottleneck, parallel]) {
    const X = indexOf(g, 'X');
    const result = minCut(g, X);
    assert.equal(result.flow, result.cutEdges.length);
    // feeding the cut into the overlay must remove X from the eager set
    const priced = evalOverrides(
      g,
      result.cutEdges.map((e) => ({ ...e, kind: 'defer' })),
    );
    assert.ok(
      priced.removed.some((m) => m.id === 'X'),
      'X should leave the initial load',
    );
  }
});

test('cut pricing frees the whole bottleneck subtree, exceeding what-if(X)', () => {
  const X = indexOf(bottleneck, 'X');
  const result = minCut(bottleneck, X);
  const priced = evalOverrides(
    bottleneck,
    result.cutEdges.map((e) => ({ ...e, kind: 'defer' })),
  );
  // cutting entry->gate frees gate + p1 + p2 + X = 10 + 20 + 20 + 1000 = 1050
  assert.equal(priced.removedBytes, 1050);
  // what-if(X) alone frees only X's retained bytes (1000)
  assert.equal(whatIf(bottleneck, X).removedBytes, 1000);
  assert.ok(priced.removedBytes > whatIf(bottleneck, X).removedBytes);
});

test('naive-minimal case: cut and what-if free the same bytes', () => {
  // star: entry imports a, b, X directly — X has one in-edge, so naive IS minimal.
  const star = makeGraph(
    [
      {
        id: 'entry',
        bytes: 5,
        imports: [
          [1, false],
          [2, false],
          [3, false],
        ],
      },
      { id: 'a', bytes: 10, imports: [] },
      { id: 'b', bytes: 20, imports: [] },
      { id: 'X', bytes: 500, imports: [] },
    ],
    ['entry'],
  );
  const X = indexOf(star, 'X');
  const result = minCut(star, X);
  assert.equal(result.flow, 1);
  assert.deepEqual(cutIds(star, result), ['entry->X']);
  const priced = evalOverrides(
    star,
    result.cutEdges.map((e) => ({ ...e, kind: 'defer' })),
  );
  assert.equal(priced.removedBytes, whatIf(star, X).removedBytes);
});

// Duplicate parallel records (the same from->to listed twice in a sidecar) must
// collapse to one unit edge — two parallel unit edges would push flow to 2 across a
// single deduped cut edge and trip the flow == |cut| invariant throw.
test('duplicate parallel edges collapse to one cut edge (no invariant throw)', () => {
  const g = makeGraph(
    [
      {
        id: 'entry',
        bytes: 5,
        imports: [
          [1, false],
          [1, false],
        ],
      },
      { id: 'X', bytes: 100, imports: [] },
    ],
    ['entry'],
  );
  const result = minCut(g, indexOf(g, 'X'));
  assert.equal(result.flow, 1);
  assert.deepEqual(cutIds(g, result), ['entry->X']);
});

// The unigraph min_cut redundant-diamond case: feat reachable via m1 and m2, cut
// nearest the sink severs both incoming edges of feat.
test('redundant diamond severs both incoming edges nearest the sink', () => {
  const g = makeGraph(
    [
      {
        id: 'root',
        bytes: 1,
        imports: [
          [1, false],
          [2, false],
        ],
      },
      { id: 'm1', bytes: 10, imports: [[3, false]] },
      { id: 'm2', bytes: 10, imports: [[3, false]] },
      { id: 'feat', bytes: 100, imports: [[4, false]] },
      { id: 'leaf', bytes: 50, imports: [] },
    ],
    ['root'],
  );
  const result = minCut(g, indexOf(g, 'feat'));
  assert.equal(result.flow, 2);
  assert.deepEqual(cutIds(g, result), ['m1->feat', 'm2->feat']);
});
