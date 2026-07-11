import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import { generateControlledCorpus } from './corpus.mjs';

const matrixPath = process.argv[2];
if (!matrixPath) throw new Error('expected a matrix JSON path');
const outputPath = process.argv[3];
const matrix = JSON.parse(await readFile(matrixPath, 'utf8'));
if (!Array.isArray(matrix.cases)) throw new Error('matrix.cases must be an array');

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
    if (options.instrumentation) {
      environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
    }
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
    throw new Error(
      `controlled transform child failed with status ${result.status}:\n${result.stderr}`,
    );
  }
  if (warmup) return;

  const peakRssMatch = result.stderr.match(/(\d+)\s+maximum resident set size/);
  if (!peakRssMatch) throw new Error('failed to parse maximum resident set size');
  const rustMetricsMatches = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-metrics\] (\{.*\})$/gm),
  ];
  const lifecycleMetrics = [
    ...result.stderr.matchAll(/^\[rolldown-parallel-plugin-init-metrics\] (\{.*\})$/gm),
  ].map((match) => JSON.parse(match[1]));
  const expectedRustMetrics = options.instrumentation && workerMatch ? 1 : 0;
  if (rustMetricsMatches.length !== expectedRustMetrics) {
    throw new Error(
      `expected ${expectedRustMetrics} Rust metrics lines for ${name}/${variant}, got ${rustMetricsMatches.length}`,
    );
  }
  const expectedLifecycleMetrics = expectedRustMetrics === 1 ? 2 : 0;
  if (lifecycleMetrics.length !== expectedLifecycleMetrics) {
    throw new Error(
      `expected ${expectedLifecycleMetrics} lifecycle metrics lines for ${name}/${variant}, got ${lifecycleMetrics.length}`,
    );
  }
  const child = JSON.parse(result.stdout);
  const rustMetrics = rustMetricsMatches[0] ? JSON.parse(rustMetricsMatches[0][1]) : undefined;
  const initializationMetrics = lifecycleMetrics.find(
    ({ kind }) => kind === 'rolldown_parallel_plugin_init_metrics',
  );
  const terminationMetrics = lifecycleMetrics.find(
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
  const { name, variants, warmups = 1, repeats = 1, ...caseOptions } = definition;
  const corpusDirectory = await mkdtemp(
    nodePath.join(tmpdir(), 'rolldown-parallel-controlled-fixture-'),
  );
  const caseRunStart = runs.length;
  try {
    const totalSourceBytes = await generateControlledCorpus({ corpusDirectory, ...caseOptions });
    const executionOptions = {
      ...caseOptions,
      _corpusDirectory: corpusDirectory,
      _totalSourceBytes: totalSourceBytes,
    };
    for (let index = 0; index < warmups; index++) {
      for (const variant of variants) {
        execute(name, executionOptions, variant, index, true);
      }
    }
    for (let index = 0; index < repeats; index++) {
      const offset = index % variants.length;
      const order = [...variants.slice(offset), ...variants.slice(0, offset)];
      for (const variant of order) {
        execute(name, executionOptions, variant, index, false);
      }
    }
    const hashes = new Set(runs.slice(caseRunStart).map((run) => run.outputHash));
    if (hashes.size !== 1) {
      throw new Error(`${name} produced different output hashes: ${[...hashes].join(', ')}`);
    }
    const rawHashes = new Set(runs.slice(caseRunStart).map((run) => run.outputRawHash));
    if (rawHashes.size !== 1) {
      throw new Error(`${name} produced different raw output hashes: ${[...rawHashes].join(', ')}`);
    }
  } finally {
    await rm(corpusDirectory, { recursive: true, force: true });
  }
}

const report = {
  schema: 1,
  startedAt,
  finishedAt: new Date().toISOString(),
  node: process.version,
  nodeBinary: process.execPath,
  matrix,
  runs,
};
const serializedReport = `${JSON.stringify(report, null, 2)}\n`;
if (outputPath) {
  await writeFile(outputPath, serializedReport);
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
  process.stdout.write(serializedReport);
}

function validateRun(run, rustMetrics, initializationMetrics, terminationMetrics, workerCount) {
  if (!run.instrumentation) {
    if (run.jsMetrics || rustMetrics || initializationMetrics || terminationMetrics) {
      throw new Error('uninstrumented run emitted metrics');
    }
    return;
  }
  const js = run.jsMetrics;
  if (!js) throw new Error('instrumented run did not emit JavaScript metrics');
  if (js.handlerCalls !== run.expectedMatchingHandlerCalls) {
    throw new Error(
      `handler call mismatch: ${js.handlerCalls} != ${run.expectedMatchingHandlerCalls}`,
    );
  }
  if (js.perWorkerCalls.reduce((total, value) => total + value, 0) !== js.handlerCalls) {
    throw new Error('per-worker handler calls do not sum to total handler calls');
  }
  if (js.factoryCalls !== Math.max(1, workerCount)) throw new Error('factory call mismatch');
  if (js.handlerActive !== 0) throw new Error('handler active count did not return to zero');
  if (js.maxHandlerActive > Math.max(1, workerCount)) {
    throw new Error('handler concurrency exceeds available instances');
  }
  const expectedWorkerMask = ((1n << BigInt(Math.max(1, workerCount))) - 1n).toString(16);
  if (js.workerMask !== expectedWorkerMask) throw new Error('worker factory mask mismatch');
  if (js.handlerInputCodeBytes !== run.totalSourceBytes) {
    throw new Error('handler input byte count does not match generated corpus');
  }
  if (workerCount === 0) {
    if (js.maxHandlerActive !== 1) throw new Error('ordinary handler concurrency is not one');
    return;
  }
  if (!rustMetrics) throw new Error('instrumented worker run did not emit Rust metrics');
  if (!initializationMetrics || !terminationMetrics) {
    throw new Error('instrumented worker run did not emit lifecycle metrics');
  }
  if (
    initializationMetrics.workerCount !== workerCount ||
    initializationMetrics.workers.length !== workerCount ||
    terminationMetrics.workerCount !== workerCount ||
    initializationMetrics.pluginCount !== 1
  ) {
    throw new Error('lifecycle worker count mismatch');
  }
  const threadNumbers = initializationMetrics.workers
    .map(({ threadNumber }) => threadNumber)
    .sort((a, b) => a - b);
  if (threadNumbers.some((value, index) => value !== index)) {
    throw new Error('lifecycle thread numbers are incomplete or duplicated');
  }
  for (const worker of initializationMetrics.workers) {
    assertDuration(worker.mainReadyMs, 'worker mainReadyMs');
    if (!worker.workerBootstrap || worker.workerBootstrap.plugins.length !== 1) {
      throw new Error('worker bootstrap plugin metrics are missing');
    }
    assertDuration(worker.workerBootstrap.measuredBootstrapMs, 'worker measuredBootstrapMs');
    assertDuration(worker.workerBootstrap.registerPluginsMs, 'worker registerPluginsMs');
    const plugin = worker.workerBootstrap.plugins[0];
    assertDuration(plugin.implementationImportMs, 'implementationImportMs');
    assertDuration(plugin.factoryMs, 'factoryMs');
    assertDuration(plugin.bindingifyMs, 'bindingifyMs');
  }
  assertDuration(initializationMetrics.poolInitializationMs, 'poolInitializationMs');
  assertDuration(terminationMetrics.poolTerminationMs, 'poolTerminationMs');
  if (rustMetrics.workerCount !== workerCount) throw new Error('Rust worker count mismatch');
  if (
    rustMetrics.wrapperCalls !== rustMetrics.permitAcquiredCalls ||
    rustMetrics.wrapperCalls !== rustMetrics.completedWrapperCalls
  ) {
    throw new Error('Rust wrapper/acquired/completed counts differ');
  }
  if (rustMetrics.valueResults !== js.handlerCalls)
    throw new Error('Rust value/JS handler counts differ');
  if (rustMetrics.nullResults !== rustMetrics.wrapperCalls - js.handlerCalls) {
    throw new Error('Rust null count does not explain wrapper-only calls');
  }
  if (rustMetrics.returnedCodeBytes !== js.handlerReturnedCodeBytes) {
    throw new Error('Rust and JavaScript returned byte counts differ');
  }
  if (rustMetrics.wrapperInputCodeBytes < js.handlerInputCodeBytes) {
    throw new Error('Rust wrapper input is smaller than matching handler input');
  }
  if (
    rustMetrics.permitQueuePending.current !== 0 ||
    rustMetrics.wrapperOutstanding.current !== 0 ||
    rustMetrics.permitInFlight.current !== 0
  ) {
    throw new Error('Rust current counters did not return to zero');
  }
  if (
    rustMetrics.errorResults !== 0 ||
    rustMetrics.cancelledBeforeAcquire !== 0 ||
    rustMetrics.cancelledDuringService !== 0
  ) {
    throw new Error('Rust metrics recorded an error or cancellation');
  }
  if (rustMetrics.permitInFlight.max > workerCount) {
    throw new Error('Rust permit concurrency exceeds worker count');
  }
}

function assertDuration(value, name) {
  if (!Number.isFinite(value) || value < 0)
    throw new Error(`${name} is not finite and non-negative`);
}
