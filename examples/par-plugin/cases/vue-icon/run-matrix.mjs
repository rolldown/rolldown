import { spawnSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { cpus, platform, release, totalmem } from 'node:os';
import nodePath from 'node:path';
import { ensureVueCorpus } from './prepare-corpus.mjs';

const matrixPath = process.argv[2];
if (!matrixPath) throw new Error('expected a matrix JSON path');
const outputPath = process.argv[3];
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
if (!Array.isArray(matrix.cases)) throw new Error('matrix.cases must be an array');

const repositoryRoot = nodePath.resolve(import.meta.dirname, '../../../..');
const git = (args) => {
  const result = spawnSync('git', ['-C', repositoryRoot, ...args], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ${args.join(' ')} failed`);
  return result.stdout.trim();
};
const corpus = await ensureVueCorpus();
const runs = [];
let sequence = 0;
const startedAt = new Date().toISOString();

const execute = (name, caseOptions, variant, index, warmup) => {
  const options = { ...caseOptions, variant };
  const environment = { ...process.env };
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
  const workerMatch = /^worker-(\d+)$/.exec(variant);
  if (workerMatch) {
    environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = workerMatch[1];
    if (options.instrumentation) environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
  }
  const result = spawnSync(
    '/usr/bin/time',
    [
      '-l',
      process.execPath,
      '--expose-gc',
      nodePath.join(import.meta.dirname, 'run-case.mjs'),
      JSON.stringify(options),
    ],
    { encoding: 'utf8', env: environment },
  );
  if (result.status !== 0) {
    throw new Error(`Vue child failed for ${name}/${variant}:\n${result.stderr}`);
  }
  if (warmup) return;

  const peakRssMatch = result.stderr.match(/(\d+)\s+maximum resident set size/);
  if (!peakRssMatch) throw new Error('failed to parse maximum resident set size');
  const rustMetricsMatches = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-metrics\] (\{.*\})$/gm),
  ];
  const lifecycle = [
    ...result.stderr.matchAll(
      /^\[rolldown-parallel-plugin-(?:init|termination)-metrics\] (\{.*\})$/gm,
    ),
  ].map((match) => JSON.parse(match[1]));
  const expectedRustMetrics = options.instrumentation && workerMatch ? 1 : 0;
  if (rustMetricsMatches.length !== expectedRustMetrics) {
    throw new Error(`unexpected Rust metrics count for ${name}/${variant}`);
  }
  if (lifecycle.length !== expectedRustMetrics * 2) {
    throw new Error(`unexpected lifecycle metrics count for ${name}/${variant}`);
  }
  const child = JSON.parse(result.stdout);
  const rustMetrics = rustMetricsMatches[0] ? JSON.parse(rustMetricsMatches[0][1]) : undefined;
  const initializationMetrics = lifecycle.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_init_metrics',
  );
  const terminationMetrics = lifecycle.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_termination_metrics',
  );
  validateRun(
    child,
    rustMetrics,
    initializationMetrics,
    terminationMetrics,
    workerMatch ? Number(workerMatch[1]) : 0,
  );
  runs.push({
    name,
    index,
    sequence: sequence++,
    peakRssBytes: Number(peakRssMatch[1]),
    ...child,
    rustMetrics,
    initializationMetrics,
    terminationMetrics,
  });
};

for (const definition of matrix.cases) {
  const { name, entrySet, variants, warmups = 1, repeats = 1, ...caseOptions } = definition;
  const selected = corpus.corpora[entrySet];
  if (!selected) throw new Error(`unknown Vue entry set: ${entrySet}`);
  const options = {
    ...caseOptions,
    _corpusDirectory: corpus.corpusDirectory,
    _entryPaths: selected.entryPaths,
    _sfcCount: selected.sfcCount,
    _totalSfcBytes: selected.totalSfcBytes,
    _manifestHash: selected.manifestHash,
  };
  const caseStart = runs.length;
  for (let index = 0; index < warmups; index++) {
    for (const variant of variants) execute(name, options, variant, index, true);
  }
  for (let index = 0; index < repeats; index++) {
    const offset = index % variants.length;
    const order = [...variants.slice(offset), ...variants.slice(0, offset)];
    for (const variant of order) execute(name, options, variant, index, false);
  }
  for (const field of ['outputRawHash', 'outputHash', 'outputBytes']) {
    const values = new Set(runs.slice(caseStart).map((run) => run[field]));
    if (values.size !== 1) throw new Error(`${name} produced different ${field} values`);
  }
}

const report = {
  schema: 1,
  startedAt,
  finishedAt: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  rolldownCommit: git(['rev-parse', 'HEAD']),
  rolldownWorktreeStatus: git(['status', '--short']),
  host: {
    platform: platform(),
    release: release(),
    architecture: process.arch,
    cpuModel: cpus()[0]?.model,
    logicalCpuCount: cpus().length,
    totalMemoryBytes: totalmem(),
  },
  corpus: {
    upstreamUrl: corpus.upstreamUrl,
    upstreamCommit: corpus.upstreamCommit,
    full: corpus.corpora.full,
    colorful: corpus.corpora.colorful,
  },
  matrix,
  runs,
};
const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await writeFile(outputPath, serialized);
  console.log(
    JSON.stringify({
      outputPath,
      cases: matrix.cases.length,
      runs: runs.length,
      startedAt,
      finishedAt: report.finishedAt,
    }),
  );
} else {
  process.stdout.write(serialized);
}

function validateRun(run, rust, initialization, termination, workerCount) {
  if (!run.instrumentation) {
    if (run.jsMetrics || rust || initialization || termination) {
      throw new Error('uninstrumented Vue run emitted metrics');
    }
    return;
  }
  if (run.variant === 'full-ordinary') throw new Error('full reference cannot be instrumented');
  const js = run.jsMetrics;
  if (!js) throw new Error('instrumented Vue run did not emit JavaScript metrics');
  if (js.handlerCalls !== run.expectedMatchingHandlerCalls)
    throw new Error('handler call mismatch');
  if (js.handlerInputCodeBytes !== run.totalSourceBytes) throw new Error('handler byte mismatch');
  if (js.handlerActive !== 0) throw new Error('handler active count did not return to zero');
  if (js.factoryCalls !== Math.max(1, workerCount)) throw new Error('factory call mismatch');
  if (js.buildStartCalls !== Math.max(1, workerCount)) throw new Error('buildStart call mismatch');
  if (js.maxHandlerActive > Math.max(1, workerCount))
    throw new Error('handler concurrency mismatch');
  if (js.perWorkerCalls.reduce((sum, value) => sum + value, 0) !== js.handlerCalls) {
    throw new Error('per-worker calls do not sum to handler calls');
  }
  const expectedMask = ((1n << BigInt(Math.max(1, workerCount))) - 1n).toString(16);
  if (js.workerMask !== expectedMask) throw new Error('worker mask mismatch');
  if (workerCount === 0) return;
  if (!rust || !initialization || !termination) throw new Error('worker metrics are incomplete');
  if (
    initialization.workerCount !== workerCount ||
    initialization.workers.length !== workerCount ||
    initialization.pluginCount !== 1 ||
    termination.workerCount !== workerCount
  ) {
    throw new Error('lifecycle worker count mismatch');
  }
  const threads = initialization.workers
    .map(({ threadNumber }) => threadNumber)
    .sort((a, b) => a - b);
  if (threads.some((thread, index) => thread !== index)) throw new Error('worker numbers mismatch');
  if (
    rust.wrapperCalls !== rust.permitAcquiredCalls ||
    rust.wrapperCalls !== rust.completedWrapperCalls ||
    rust.valueResults !== js.handlerCalls ||
    rust.nullResults !== rust.wrapperCalls - js.handlerCalls ||
    rust.returnedCodeBytes !== js.handlerReturnedCodeBytes
  ) {
    throw new Error('Rust and JavaScript transform counts differ');
  }
  if (rust.wrapperInputCodeBytes < js.handlerInputCodeBytes)
    throw new Error('Rust input byte mismatch');
  if (
    rust.permitQueuePending.current !== 0 ||
    rust.wrapperOutstanding.current !== 0 ||
    rust.permitInFlight.current !== 0 ||
    rust.permitInFlight.max > workerCount ||
    rust.errorResults !== 0 ||
    rust.cancelledBeforeAcquire !== 0 ||
    rust.cancelledDuringService !== 0
  ) {
    throw new Error('Rust counters are invalid');
  }
}
