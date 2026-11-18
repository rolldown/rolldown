import { test } from 'node:test';
import assert from 'node:assert/strict';

import { comparePaints, renderGapEvidence } from '../lib/paint-diagnostics.mjs';
import { summarize } from '../lib/measure.mjs';

const noise = { noiseFloorMs: 30, noiseFloorPct: 2 };
const deltas = (fcp, lcp) => ({
  'runtime.fcp_ms': { prev: 1000, delta: fcp },
  'runtime.lcp_ms': { prev: 2000, delta: lcp },
});

test('an FCP gain counts when LCP stays within noise', () => {
  const result = comparePaints(deltas(-100, 20), noise);
  assert.equal(result.state, 'improved');
  assert.equal(result.metrics.fcp.state, 'improved');
  assert.equal(result.metrics.lcp.state, 'within-noise');
});

test('a regression in either paint prevents a keep recommendation', () => {
  assert.equal(comparePaints(deltas(-200, 60), noise).state, 'regressed');
  assert.equal(comparePaints(deltas(50, -400), noise).state, 'regressed');
  assert.equal(comparePaints(deltas(-29, -39), noise).state, 'within-noise');
});

test('both metrics are required to judge a gain', () => {
  const result = comparePaints({ 'runtime.lcp_ms': { prev: 2000, delta: -400 } }, noise);
  assert.equal(result.state, 'unknown');
  assert.equal(result.metrics.fcp.state, 'unknown');
});

test('missing FCP in one run fails the paint guard even when its median exists', () => {
  const result = summarize(
    [
      { fcp: 1000, lcp: 2000, resources: [] },
      { fcp: null, lcp: 2000, resources: [] },
    ],
    [],
  );
  assert.equal(result.metrics['runtime.fcp_ms'], 1000);
  assert.equal(result.guard.fcpObservedInAllRuns, false);
  assert.equal(result.guard.lcpObservedInAllRuns, true);
});

test('script candidates preserve concurrent fetch, font, and image evidence', () => {
  const evidence = renderGapEvidence({
    postLoadScripts: {
      fcp: { count: 1, bytes: 1024 },
      lcp: { count: 2, bytes: 2048 },
    },
    gatingFetches: ['/api/content (fetch, finished 20ms before first paint)'],
    resourceWeight: [
      { type: 'font', preFcpCount: 1, preFcpKb: 60 },
      { type: 'image', preFcpCount: 1, preFcpKb: 200 },
    ],
  });
  assert.deepEqual(
    evidence.map((e) => [e.kind, e.paint]),
    [
      ['script', 'fcp'],
      ['script', 'lcp'],
      ['fetch', 'fcp'],
      ['font', 'fcp'],
      ['image', 'fcp'],
    ],
  );
  assert.match(evidence.find((e) => e.kind === 'fetch').detail, /\/api\/content/);
  assert.match(evidence.find((e) => e.kind === 'image').next, /Keep its request early/);
});
