import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { FROZEN_SELECTIONS } from './corpus.mjs';
import { assertLocalExecution } from './provenance.mjs';
import { classifyMonotonicScreen, confirmationWorkerCounts } from './statistics-policy.mjs';

assertLocalExecution();
const screenReportPath = process.argv[2];
const outputPath = process.argv[3];
if (!screenReportPath || !outputPath) {
  throw new Error('expected <wall-screen-report.json> <wall-confirm-matrix.json>');
}
const screenContent = await readFile(screenReportPath);
const report = JSON.parse(screenContent);
if (
  report.matrix?.lane !== 'wall-screen' ||
  report.measurementClass !== 'formal local wall evidence subject to host gates'
) {
  throw new Error('confirmation matrix requires a formal Vue wall-screen report');
}
if (
  report.admitted !== true ||
  report.admissionFailures?.length !== 0 ||
  !report.evidence?.admission ||
  !report.evidence?.correctness
) {
  throw new Error('confirmation matrix requires an admitted evidence-bound Vue wall screen');
}

const scales = Object.keys(FROZEN_SELECTIONS).map(Number);
const screens = scales.map((componentCount) => {
  const runs = report.runs.filter((run) => run.componentCount === componentCount);
  const ordinary = runs.find((run) => run.variant === 'ordinary');
  const workers = runs.filter((run) => /^worker-[1-8]$/.test(run.variant));
  if (!ordinary || workers.length !== 8 || runs.length !== 9) {
    throw new Error(`screen is incomplete at scale ${componentCount}`);
  }
  const bestWorker = workers.reduce((best, run) =>
    run.totalElapsedMs < best.totalElapsedMs ||
    (run.totalElapsedMs === best.totalElapsedMs &&
      workerCount(run.variant) < workerCount(best.variant))
      ? run
      : best,
  );
  return {
    componentCount,
    ordinaryElapsedMs: ordinary.totalElapsedMs,
    bestWorkerVariant: bestWorker.variant,
    bestWorkerElapsedMs: bestWorker.totalElapsedMs,
    speedup: ordinary.totalElapsedMs / bestWorker.totalElapsedMs,
    belowTwoSeconds: Math.max(ordinary.totalElapsedMs, bestWorker.totalElapsedMs) < 2000,
  };
});

const { outcome, selectedScales } = classifyMonotonicScreen(screens);

const uniqueScales = [...new Set(selectedScales)];
const cases = uniqueScales.map((componentCount, index) => {
  const screen = screens.find((entry) => entry.componentCount === componentCount);
  const bestCount = workerCount(screen.bestWorkerVariant);
  const counts = confirmationWorkerCounts(bestCount);
  return {
    name: `vue-scale-${componentCount}-confirm`,
    componentCount,
    variants: ['ordinary', ...counts.map((count) => `worker-${count}`)],
    repeats: screen.belowTwoSeconds ? 15 : 10,
    rotationOffset: index,
    instrumentation: false,
    auditSources: false,
  };
});

const matrix = {
  schema: 1,
  lane: 'wall-confirm',
  description:
    'Generated from one formal screen. It repeats the first direction-change neighbors and full scale, or the frozen boundary points when the screen does not bracket a crossover.',
  bindingProfile: 'release',
  runtimePin: report.matrix.runtimePin,
  configuredPools: { tokio: 18, rayon: 12, blocking: 4 },
  requiredEvidence: report.matrix.requiredEvidence,
  provenanceLock: {
    harnessAggregateSha256: report.harnessSourceManifest.aggregateSha256,
    vueToolchain: report.vueToolchain,
    corpusAggregateSha256: report.corpus.summary.aggregateSha256,
    runtimePin: report.runtime.runtimePin,
    evidence: report.evidence,
  },
  generatedFrom: {
    path: nodePath.resolve(screenReportPath),
    sha256: createHash('sha256').update(screenContent).digest('hex'),
    startedAt: report.startedAt,
    finishedAt: report.finishedAt,
    outcome,
    screens,
  },
  cases,
};
await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(matrix, null, 2)}\n`);
console.log(JSON.stringify({ outputPath, outcome, scales: uniqueScales, cases: cases.length }));

function workerCount(variant) {
  return Number(variant.slice('worker-'.length));
}
