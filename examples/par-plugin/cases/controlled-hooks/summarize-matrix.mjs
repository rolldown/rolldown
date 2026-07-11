import { readFile, writeFile } from 'node:fs/promises';

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
    hook: runs[0].hook,
    variant,
    samples: runs.length,
    totalElapsedMs: statistics(runs.map((run) => run.totalElapsedMs)),
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
    handlerInputBytesPerCall: optionalStatistics(
      runs.map((run) =>
        run.jsMetrics ? run.jsMetrics.handlerInputBytes / run.jsMetrics.handlerCalls : undefined,
      ),
    ),
    handlerReturnedBytesPerCall: optionalStatistics(
      runs.map((run) =>
        run.jsMetrics ? run.jsMetrics.handlerReturnedBytes / run.jsMetrics.handlerCalls : undefined,
      ),
    ),
    permitHeldNsPerWrapperCall: optionalStatistics(
      runs.map((run) =>
        run.rustMetrics
          ? run.rustMetrics.permitHeldNs.total / run.rustMetrics.wrapperCalls
          : undefined,
      ),
    ),
    permitQueueWaitNsPerWrapperCall: optionalStatistics(
      runs.map((run) =>
        run.rustMetrics
          ? run.rustMetrics.permitQueueWaitNs.total / run.rustMetrics.wrapperCalls
          : undefined,
      ),
    ),
    nativeFilterMisses: optionalStatistics(runs.map((run) => run.rustMetrics?.nullResults)),
    maxHandlerActive: optionalStatistics(runs.map((run) => run.jsMetrics?.maxHandlerActive)),
    maxPermitInFlight: optionalStatistics(runs.map((run) => run.rustMetrics?.permitInFlight.max)),
    maxWrapperOutstanding: optionalStatistics(
      runs.map((run) => run.rustMetrics?.wrapperOutstanding.max),
    ),
    poolInitializationMs: optionalStatistics(
      runs.map((run) => run.initializationMetrics?.poolInitializationMs),
    ),
    outputHashes: [...new Set(runs.map((run) => run.outputHash))],
  });
}

summaries.sort(
  (a, b) => a.name.localeCompare(b.name) || variantOrder(a.variant) - variantOrder(b.variant),
);
const summary = {
  schema: 1,
  sourceReport: reportPath,
  sourceStartedAt: report.startedAt,
  sourceFinishedAt: report.finishedAt,
  sourceEnvironment: {
    node: report.node,
    rolldownCommit: report.rolldownCommit,
    nativeBinding: report.nativeBinding,
    host: report.host,
  },
  matrix: report.matrix,
  summaries,
};
const serialized = `${JSON.stringify(summary, null, 2)}\n`;
if (outputPath) {
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
  const sorted = [...values].sort((a, b) => a - b);
  const mean = values.reduce((total, value) => total + value, 0) / values.length;
  const median = quantile(sorted, 0.5);
  const squaredDeviation = values.reduce((total, value) => total + (value - mean) ** 2, 0);
  return {
    n: values.length,
    mean,
    median,
    sampleStddev: values.length > 1 ? Math.sqrt(squaredDeviation / (values.length - 1)) : 0,
    mad: quantile(
      values.map((value) => Math.abs(value - median)).sort((a, b) => a - b),
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
