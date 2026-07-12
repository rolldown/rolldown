import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { FROZEN_SELECTIONS } from './corpus.mjs';
import { assertLocalExecution } from './provenance.mjs';
import {
  RESOURCE_LIMITS,
  confirmationWorkerCounts,
  createPolicyEvidence,
  isMechanicalGain,
  isResourceEligible,
  resolveConfirmedCrossover,
  selectWorkerWithTieRule,
} from './statistics-policy.mjs';

assertLocalExecution();
const reportPath = process.argv[2];
const outputPath = process.argv[3];
if (!reportPath) throw new Error('expected a repeated wall-confirm report path');
const reportContent = await readFile(reportPath);
const report = JSON.parse(reportContent);
if (
  report.matrix?.lane !== 'wall-confirm' ||
  report.measurementClass !== 'formal local wall evidence subject to host gates' ||
  !report.admitted ||
  report.admissionFailures?.length !== 0
) {
  throw new Error('Vue scale summary requires an admitted formal wall-confirm report');
}

const BOOTSTRAP_RESAMPLES = 100_000;
const BOOTSTRAP_SEED = 0x20260712;
const runsByScale = Map.groupBy(report.runs, (run) => run.componentCount);
const scales = [...runsByScale.keys()].sort((left, right) => left - right);
const scaleSummaries = [];

for (const componentCount of scales) {
  const runs = runsByScale.get(componentCount);
  const variants = [...new Set(runs.map((run) => run.variant))];
  const ordinaryByRound = new Map(
    runs.filter((run) => run.variant === 'ordinary').map((run) => [run.index, run]),
  );
  if (ordinaryByRound.size === 0) throw new Error(`ordinary is missing at ${componentCount}`);
  const variantSummaries = variants.map((variant) => {
    const variantRuns = runs.filter((run) => run.variant === variant);
    if (variantRuns.length !== ordinaryByRound.size) {
      throw new Error(`incomplete rotated blocks for ${componentCount}/${variant}`);
    }
    const walls = variantRuns.map((run) => run.totalElapsedMs);
    const cpu = variantRuns.map((run) => run.cpuUserMs + run.cpuSystemMs);
    const rss = variantRuns.map((run) => run.peakRssBytes);
    const paired = variantRuns.map((run) => {
      const ordinary = ordinaryByRound.get(run.index);
      if (!ordinary) throw new Error(`missing paired ordinary round ${run.index}`);
      return {
        speedup: ordinary.totalElapsedMs / run.totalElapsedMs,
        wallRatio: run.totalElapsedMs / ordinary.totalElapsedMs,
        cpuRatio: (run.cpuUserMs + run.cpuSystemMs) / (ordinary.cpuUserMs + ordinary.cpuSystemMs),
        rssRatio: run.peakRssBytes / ordinary.peakRssBytes,
      };
    });
    const label = `${componentCount}/${variant}`;
    return {
      variant,
      workerCount: variant === 'ordinary' ? 0 : Number(variant.slice('worker-'.length)),
      samples: variantRuns.length,
      wallMs: statistics(walls),
      wallMedianBootstrap95: bootstrapMedianInterval(walls, `${label}/wall`),
      totalCpuMs: statistics(cpu),
      peakRssBytes: statistics(rss),
      pairedSpeedup: statistics(paired.map((sample) => sample.speedup)),
      pairedSpeedupBootstrap95: bootstrapMedianInterval(
        paired.map((sample) => sample.speedup),
        `${label}/speedup`,
      ),
      pairedWallRatio: statistics(paired.map((sample) => sample.wallRatio)),
      pairedWallRatioBootstrap95: bootstrapMedianInterval(
        paired.map((sample) => sample.wallRatio),
        `${label}/wall-ratio`,
      ),
      pairedCpuRatio: statistics(paired.map((sample) => sample.cpuRatio)),
      pairedRssRatio: statistics(paired.map((sample) => sample.rssRatio)),
      outputCodeHashes: [...new Set(variantRuns.map((run) => run.outputCodeHash))],
      outputMapHashes: [...new Set(variantRuns.map((run) => run.outputMapHash))],
      pagingDeltas: [...new Set(variantRuns.map((run) => JSON.stringify(run.pagingDelta)))],
    };
  });
  const ordinary = variantSummaries.find((summary) => summary.variant === 'ordinary');
  const workers = variantSummaries.filter((summary) => summary.workerCount > 0);
  if (!ordinary || workers.length === 0) {
    throw new Error(`confirmation variants are incomplete at ${componentCount}`);
  }
  for (const worker of workers) {
    worker.mechanicalGain = isMechanicalGain(worker);
    worker.resourceEligible = isResourceEligible(worker);
  }
  const selectedWorker = selectWorkerWithTieRule(workers);
  const resourceCandidates = workers.filter(({ resourceEligible }) => resourceEligible);
  const selectedResourceWorker =
    resourceCandidates.length === 0 ? null : selectWorkerWithTieRule(resourceCandidates);
  scaleSummaries.push({
    componentCount,
    selectedWorker: selectedWorker.variant,
    selectedWorkerCount: selectedWorker.workerCount,
    selectedResourceWorker: selectedResourceWorker?.variant ?? null,
    selectedResourceWorkerCount: selectedResourceWorker?.workerCount ?? null,
    tieRule:
      'choose the smaller confirmed count when wall-median bootstrap intervals overlap and median wall differs by less than 2%',
    mechanicalGain: selectedWorker.mechanicalGain,
    resourceEligible: selectedResourceWorker !== null,
    variants: variantSummaries,
  });
}

const frozenScales = Object.keys(FROZEN_SELECTIONS).map(Number);
const mechanicalResolution = decorateResolution(
  resolveConfirmedCrossover(scaleSummaries, 'mechanicalGain', frozenScales),
  'selectedWorker',
);
const resourceResolution = decorateResolution(
  resolveConfirmedCrossover(scaleSummaries, 'resourceEligible', frozenScales),
  'selectedResourceWorker',
);
const additionalConfirmationMatrix = createAdditionalConfirmationMatrix([
  mechanicalResolution,
  resourceResolution,
]);
const policyEvidence = createPolicyEvidence(scaleSummaries);
const summary = {
  schema: 1,
  sourceReport: reportPath,
  sourceReportSha256: createHash('sha256').update(reportContent).digest('hex'),
  sourceStartedAt: report.startedAt,
  sourceFinishedAt: report.finishedAt,
  runtime: report.runtime,
  fixture: report.fixture,
  corpus: report.corpus,
  statisticsProtocol: {
    pairedBy: 'componentCount and rotated-block index',
    estimator: 'paired median ordinaryWall / workerWall',
    policyWallRatioEstimator: 'paired median workerWall / ordinaryWall',
    bootstrapResamples: BOOTSTRAP_RESAMPLES,
    bootstrapSeed: `0x${BOOTSTRAP_SEED.toString(16)}`,
    interval: 'deterministic percentile bootstrap 95%',
  },
  resourceGate: {
    ...RESOURCE_LIMITS,
    requiredPageoutAndSwapoutDelta: 0,
  },
  mechanicalCrossover: mechanicalResolution,
  resourceAcceptableCrossover: resourceResolution,
  additionalConfirmationMatrix,
  policyEvidence,
  scaleSummaries,
};
const serialized = `${JSON.stringify(summary, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(
    JSON.stringify({
      outputPath,
      scales: scaleSummaries.length,
      mechanicalCrossover: mechanicalResolution,
      resourceAcceptableCrossover: resourceResolution,
      additionalConfirmationRequired: additionalConfirmationMatrix !== null,
    }),
  );
} else {
  process.stdout.write(serialized);
}

function decorateResolution(resolution, workerField) {
  if (!resolution.crossover?.componentCount) return resolution;
  const scale = scaleSummaries.find(
    ({ componentCount }) => componentCount === resolution.crossover.componentCount,
  );
  return {
    ...resolution,
    crossover: { ...resolution.crossover, selectedWorker: scale?.[workerField] ?? null },
  };
}

function createAdditionalConfirmationMatrix(resolutions) {
  const requestedScales = new Set();
  for (const resolution of resolutions) {
    for (const scale of resolution.additionalScales) requestedScales.add(scale);
    for (const scale of resolution.repeatScales) requestedScales.add(scale);
  }
  if (requestedScales.size === 0) return null;
  const screens = report.matrix.generatedFrom?.screens;
  if (!Array.isArray(screens)) {
    throw new Error('iterative confirmation requires the pinned source screen summaries');
  }
  const cases = [...requestedScales]
    .sort((left, right) => left - right)
    .map((componentCount, index) => {
      const screen = screens.find((entry) => entry.componentCount === componentCount);
      if (!screen) throw new Error(`missing source screen at ${componentCount}`);
      const bestCount = Number(screen.bestWorkerVariant.slice('worker-'.length));
      const counts = confirmationWorkerCounts(bestCount);
      return {
        name: `vue-scale-${componentCount}-additional-confirm`,
        componentCount,
        variants: ['ordinary', ...counts.map((count) => `worker-${count}`)],
        repeats: screen.belowTwoSeconds ? 15 : 10,
        rotationOffset: index,
        instrumentation: false,
        auditSources: false,
      };
    });
  return {
    schema: 1,
    lane: 'wall-confirm',
    description:
      'Iterative confirmation requested because repeated evidence did not establish an actual-adjacent frozen crossover boundary.',
    bindingProfile: 'release',
    runtimePin: report.matrix.runtimePin,
    configuredPools: report.matrix.configuredPools,
    requiredEvidence: report.matrix.requiredEvidence,
    provenanceLock: report.matrix.provenanceLock,
    generatedFrom: {
      priorConfirmationPath: nodePath.resolve(reportPath),
      priorConfirmationSha256: createHash('sha256').update(reportContent).digest('hex'),
      sourceScreen: report.matrix.generatedFrom,
      resolutions,
    },
    cases,
  };
}

function bootstrapMedianInterval(values, label) {
  const random = xorshift32(BOOTSTRAP_SEED ^ hashLabel(label));
  const medians = new Float64Array(BOOTSTRAP_RESAMPLES);
  const sample = Array.from({ length: values.length });
  for (let iteration = 0; iteration < BOOTSTRAP_RESAMPLES; iteration++) {
    for (let index = 0; index < values.length; index++) {
      sample[index] = values[Math.floor(random() * values.length)];
    }
    medians[iteration] = median(sample);
  }
  medians.sort();
  return {
    lower: quantile(medians, 0.025),
    upper: quantile(medians, 0.975),
  };
}

function xorshift32(seed) {
  let state = seed >>> 0 || 0x6d2b79f5;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 2 ** 32;
  };
}

function hashLabel(value) {
  let hash = 0x811c9dc5;
  for (const byte of Buffer.from(value)) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function statistics(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const mean = values.reduce((total, value) => total + value, 0) / values.length;
  return {
    n: values.length,
    mean,
    median: quantile(sorted, 0.5),
    min: sorted[0],
    max: sorted.at(-1),
  };
}

function median(values) {
  return quantile(
    [...values].sort((left, right) => left - right),
    0.5,
  );
}

function quantile(sorted, probability) {
  if (sorted.length === 1) return sorted[0];
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const fraction = position - lower;
  return (
    sorted[lower] + (sorted[Math.min(lower + 1, sorted.length - 1)] - sorted[lower]) * fraction
  );
}
