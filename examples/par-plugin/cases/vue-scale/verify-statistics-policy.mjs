import assert from 'node:assert/strict';
import {
  classifyMonotonicScreen,
  confirmationWorkerCounts,
  createPolicyEvidence,
  isResourceEligible,
  resolveConfirmedCrossover,
  selectWorkerWithTieRule,
} from './statistics-policy.mjs';

const scales = [32, 128, 256, 512, 1024, 2048, 4096, 5000];
const screens = (speedups) =>
  speedups.map((speedup, index) => ({ componentCount: scales[index], speedup }));

assert.deepEqual(confirmationWorkerCounts(1), [1, 2, 4, 8]);
assert.deepEqual(confirmationWorkerCounts(4), [3, 4, 5, 8]);
assert.deepEqual(confirmationWorkerCounts(8), [4, 7, 8]);
assert.throws(() => confirmationWorkerCounts(0));

assert.deepEqual(
  createPolicyEvidence([
    {
      componentCount: 512,
      selectedWorkerCount: 4,
      selectedResourceWorkerCount: 4,
      variants: [
        {
          variant: 'ordinary',
          workerCount: 0,
          wallMs: { median: 100 },
          totalCpuMs: { median: 80 },
          peakRssBytes: { median: 1000 },
          pairedWallRatioBootstrap95: { upper: 1 },
        },
        {
          variant: 'worker-4',
          workerCount: 4,
          wallMs: { median: 60 },
          totalCpuMs: { median: 180 },
          peakRssBytes: { median: 2000 },
          resourceEligible: true,
          pairedWallRatioBootstrap95: { upper: 0.65 },
        },
      ],
    },
  ]),
  {
    schema: 1,
    jsonPointerBase: '/policyEvidence/byScale',
    byScale: {
      512: {
        variants: {
          ordinary: {
            wallMedianMs: 100,
            cpuMedianMs: 80,
            peakRssMedianBytes: 1000,
            resourceEligible: true,
            pairedWallRatioBootstrap95Upper: 1,
            selectedOracleCount: 4,
          },
          'worker-4': {
            wallMedianMs: 60,
            cpuMedianMs: 180,
            peakRssMedianBytes: 2000,
            resourceEligible: true,
            pairedWallRatioBootstrap95Upper: 0.65,
            selectedOracleCount: 4,
          },
        },
      },
    },
  },
);

assert.deepEqual(classifyMonotonicScreen(screens([0.9, 0.95, 1.01, 1.1, 1.2, 1.3, 1.4, 1.5])), {
  outcome: 'screen-bracketed-first-positive-direction-change',
  selectedScales: [128, 256, 512, 5000],
});
assert.throws(
  () => classifyMonotonicScreen(screens([0.9, 1.01, 0.99, 1.1, 1.2, 1.3, 1.4, 1.5])),
  /non-monotonic/,
);
assert.throws(
  () => classifyMonotonicScreen(screens([1.01, 0.99, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6])),
  /non-monotonic/,
);

const tieCandidate = (workerCount, median, lower, upper) => ({
  variant: `worker-${workerCount}`,
  workerCount,
  wallMs: { median },
  wallMedianBootstrap95: { lower, upper },
});
assert.equal(
  selectWorkerWithTieRule([
    tieCandidate(2, 101, 99, 103),
    tieCandidate(3, 100.5, 99, 102),
    tieCandidate(4, 100, 99, 101),
  ]).workerCount,
  2,
);

const resourceCandidate = (workerCount, wallMedian, cpuRatio) => ({
  ...tieCandidate(workerCount, wallMedian, wallMedian - 1, wallMedian + 1),
  pairedSpeedup: { median: 1.2 },
  pairedSpeedupBootstrap95: { lower: 1.1, upper: 1.3 },
  pairedCpuRatio: { median: cpuRatio },
  pairedRssRatio: { median: 1.2 },
  peakRssBytes: { max: 2 * 1024 ** 3 },
  pagingDeltas: [JSON.stringify({ pageouts: 0, swapouts: 0 })],
  outputCodeHashes: ['code'],
  outputMapHashes: ['map'],
});
const fastestButIneligible = resourceCandidate(8, 90, 2.1);
const slowerEligible = resourceCandidate(4, 100, 1.5);
assert.equal(isResourceEligible(fastestButIneligible), false);
assert.equal(isResourceEligible(slowerEligible), true);
assert.equal(
  selectWorkerWithTieRule(
    [fastestButIneligible, slowerEligible].filter((worker) => isResourceEligible(worker)),
  ).workerCount,
  4,
);

const summaries = (values) =>
  Object.entries(values).map(([componentCount, value]) => ({
    componentCount: Number(componentCount),
    gain: value,
  }));
assert.deepEqual(
  resolveConfirmedCrossover(summaries({ 128: false, 256: true, 512: true }), 'gain', scales),
  {
    status: 'confirmed',
    crossover: { componentCount: 256, confirmedByComponentCount: 512 },
    additionalScales: [],
    repeatScales: [],
  },
);
assert.deepEqual(
  resolveConfirmedCrossover(summaries({ 128: true, 256: true, 512: true }), 'gain', scales),
  {
    status: 'additional-confirmation-required',
    crossover: null,
    additionalScales: [32],
    repeatScales: [],
    reason: 'the immediately smaller frozen scale before 128 was not repeated',
  },
);
assert.equal(
  resolveConfirmedCrossover(summaries({ 32: true, 128: true }), 'gain', scales).status,
  'left-censored',
);
assert.deepEqual(
  resolveConfirmedCrossover(summaries({ 512: true, 5000: true }), 'gain', scales).additionalScales,
  [256, 1024],
);
assert.equal(
  resolveConfirmedCrossover(summaries({ 4096: false, 5000: false }), 'gain', scales).status,
  'not-observed-through-maximum',
);
assert.equal(
  resolveConfirmedCrossover(summaries({ 4096: false, 5000: true }), 'gain', scales).status,
  'right-boundary-unconfirmed',
);
assert.equal(
  resolveConfirmedCrossover(summaries({ 128: true, 256: false }), 'gain', scales).status,
  'inconsistent-repeated-direction',
);

console.log(JSON.stringify({ verified: true, scenarios: 17 }));
