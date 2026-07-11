import { mkdir, readFile, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';

const reportPath = process.argv[2];
if (!reportPath) throw new Error('expected a raw matrix report path');
const outputPath = process.argv[3];
const report = JSON.parse(await readFile(reportPath, 'utf8'));
const groups = Map.groupBy(report.runs, (run) => `${run.name}\0${run.variant}`);
const ordinaryByCaseAndRound = new Map(
  report.runs
    .filter((run) => run.variant === 'ordinary')
    .map((run) => [`${run.name}\0${run.index}`, run]),
);
const summaries = [];

for (const [key, runs] of groups) {
  const [name, variant] = key.split('\0');
  const pairedOrdinary = runs.map((run) => ordinaryByCaseAndRound.get(`${name}\0${run.index}`));
  const hasPairedOrdinary = pairedOrdinary.every(Boolean);
  summaries.push({
    name,
    variant,
    samples: runs.length,
    totalElapsedMs: statistics(runs.map((run) => run.totalElapsedMs)),
    pluginSetupElapsedMs: statistics(runs.map((run) => run.pluginSetupElapsedMs)),
    rolldownApiElapsedMs: statistics(runs.map((run) => run.rolldownApiElapsedMs)),
    cpuUserMs: statistics(runs.map((run) => run.cpuUserMs)),
    peakRssBytes: statistics(runs.map((run) => run.peakRssBytes)),
    pairedSpeedupVsOrdinary:
      variant === 'ordinary'
        ? statistics(runs.map(() => 1))
        : hasPairedOrdinary
          ? statistics(
              runs.map((run, index) => pairedOrdinary[index].totalElapsedMs / run.totalElapsedMs),
            )
          : undefined,
    handlerNsPerCall: optionalStatistics(
      runs.map((run) =>
        run.jsMetrics ? run.jsMetrics.handlerNsTotal / run.jsMetrics.handlerCalls : undefined,
      ),
    ),
    maxHandlerActive: optionalStatistics(runs.map((run) => run.jsMetrics?.maxHandlerActive)),
    maxPermitInFlight: optionalStatistics(runs.map((run) => run.rustMetrics?.permitInFlight.max)),
    maxWrapperOutstanding: optionalStatistics(
      runs.map((run) => run.rustMetrics?.wrapperOutstanding.max),
    ),
    poolInitializationMs: optionalStatistics(
      runs.map((run) => run.initializationMetrics?.poolInitializationMs),
    ),
    workerImplementationImportMs: optionalStatistics(
      runs.flatMap(
        (run) =>
          run.initializationMetrics?.workers.flatMap((worker) =>
            worker.workerBootstrap.plugins.map((plugin) => plugin.implementationImportMs),
          ) ?? [],
      ),
    ),
    outputCodeHashes: [...new Set(runs.map((run) => run.outputCodeHash))],
    outputMapHashes: [...new Set(runs.map((run) => run.outputMapHash))],
  });
}

summaries.sort(
  (left, right) =>
    left.name.localeCompare(right.name) || variantOrder(left.variant) - variantOrder(right.variant),
);
const summary = {
  schema: 1,
  sourceReport: reportPath,
  sourceStartedAt: report.startedAt,
  sourceFinishedAt: report.finishedAt,
  node: report.node,
  nodeBinary: report.nodeBinary,
  svelteVersion: report.svelteVersion,
  rolldownCommit: report.rolldownCommit,
  rolldownWorktreeStatus: report.rolldownWorktreeStatus,
  nativeBinding: report.nativeBinding,
  host: report.host,
  corpus: report.corpus,
  matrix: report.matrix,
  caseSelections: report.caseSelections,
  summaries,
};
const serialized = `${JSON.stringify(summary, null, 2)}\n`;
if (outputPath) {
  await mkdir(nodePath.dirname(nodePath.resolve(outputPath)), { recursive: true });
  await writeFile(outputPath, serialized);
  console.log(JSON.stringify({ outputPath, summaries: summaries.length }));
} else {
  process.stdout.write(serialized);
}

function optionalStatistics(values) {
  const defined = values.filter((value) => value !== undefined);
  return defined.length === 0 ? undefined : statistics(defined);
}

function statistics(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const mean = values.reduce((total, value) => total + value, 0) / values.length;
  const median = quantile(sorted, 0.5);
  const squaredDeviation = values.reduce((total, value) => total + (value - mean) ** 2, 0);
  return {
    n: values.length,
    mean,
    median,
    sampleStddev: values.length > 1 ? Math.sqrt(squaredDeviation / (values.length - 1)) : 0,
    mad: quantile(
      values.map((value) => Math.abs(value - median)).sort((left, right) => left - right),
      0.5,
    ),
    q1: quantile(sorted, 0.25),
    q3: quantile(sorted, 0.75),
    min: sorted[0],
    max: sorted.at(-1),
  };
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

function variantOrder(variant) {
  if (variant === 'ordinary') return 0;
  return Number(variant.slice('worker-'.length));
}
