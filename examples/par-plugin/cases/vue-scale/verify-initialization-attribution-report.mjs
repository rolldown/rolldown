import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { validateInitializationAttributionBundle } from './initialization-attribution-validation.mjs';
import { assertLocalExecution } from './provenance.mjs';

assertLocalExecution();
const reportPath = process.argv[2];
if (!reportPath) throw new Error('expected an attribution contract report path');
const report = JSON.parse(await readFile(reportPath, 'utf8'));
if (
  report.schema !== 1 ||
  report.matrix?.lane !== 'attribution-contract-smoke' ||
  report.measurementClass !== 'untimed attribution contract validation; not performance evidence' ||
  !Array.isArray(report.runs) ||
  report.runs.length !== 2
) {
  throw new Error('expected one ordinary/worker-4 attribution contract report');
}

const ordinary = report.runs.find(({ variant }) => variant === 'ordinary');
const worker = report.runs.find(({ variant }) => variant === 'worker-4');
if (!ordinary || !worker) throw new Error('attribution contract report variants differ');
validate(ordinary);
validate(worker);

const negatives = [
  [ordinary, (run) => run.nativePluginRegistrationMetrics.metricsId++, /metricsId differ/],
  [
    ordinary,
    (run) => run.createBundlerOptionsMetrics.pluginCounts.ordinaryJs++,
    /plugin counts differ/,
  ],
  [
    ordinary,
    (run) => run.createBundlerOptionsMetrics.pluginBinding[1].pluginIndex++,
    /identity or stage/,
  ],
  [worker, (run) => run.initializationMetrics.metricsId++, /header, identity, or plugin indexes/],
  [
    worker,
    (run) => run.initializationMetrics.parallelPluginIndexes[0]--,
    /header, identity, or plugin indexes/,
  ],
  [
    worker,
    (run) => run.initializationMetrics.workers[0].workerBootstrap.metricsId++,
    /bootstrap header/,
  ],
  [
    worker,
    (run) => run.initializationMetrics.workers[0].workerBootstrap.launcher.metricsId++,
    /launcher header/,
  ],
  [
    worker,
    (run) =>
      (run.initializationMetrics.cpuWindows.residualProcessCpuDeltaMicros = { user: 0, system: 0 }),
    /CPU windows keys differ/,
  ],
  [
    worker,
    (run) => run.initializationMetrics.cpuWindows.outerProcessWindow.startedAt.monotonicMs++,
    /exact process snapshot endpoints|clock origin/,
  ],
  [
    worker,
    (run) =>
      (run.initializationMetrics.cpuWindows.workerSamples[0].endBounds.latestAt.monotonicMs += 1e6),
    /outside its declared bounds|clock origin/,
  ],
  [
    worker,
    (run) =>
      run.initializationMetrics.workers[0].workerBootstrap.launcher.stages.metricsRuntimeImport
        .durationMs++,
    /duration differs/,
  ],
  [
    worker,
    (run) => run.nativePluginRegistrationMetrics.workerManagerWorkerCount++,
    /header or counts|WorkerManager count/,
  ],
  [
    ordinary,
    (run) =>
      (run.createBundlerOptionsMetrics.resources.scope =
        'whole-process RSS is owned by the current plugin'),
    /resource scope or RSS ownership claim/,
  ],
  [
    ordinary,
    (run) =>
      (run.createBundlerOptionsMetrics.resources.afterPluginNormalization.scope.memoryUsage =
        'whole-process RSS is owned by the main isolate'),
    /scope or RSS ownership claim/,
  ],
  [
    worker,
    (run) =>
      (run.initializationMetrics.workers[0].workerBootstrap.launcher.resources.afterRuntimeAndBindingImport.scope.memoryUsage =
        'whole-process RSS is owned exclusively by worker 0'),
    /scope or RSS ownership claim/,
  ],
  [
    worker,
    (run) =>
      (run.initializationMetrics.workers[0].workerBootstrap.workerLocalAtReady.scope.memoryUsage =
        'whole-process RSS is owned exclusively by this worker'),
    /scope or RSS ownership claim/,
  ],
];
for (const [source, mutate, pattern] of negatives) {
  const copy = structuredClone(source);
  mutate(copy);
  assert.throws(() => validate(copy), pattern);
}

console.log(`Vue initialization attribution report passed ${negatives.length} negative cases`);

function validate(run) {
  const workerCount = run.variant === 'ordinary' ? 0 : Number(run.variant.slice('worker-'.length));
  validateInitializationAttributionBundle({
    createBundlerOptions: run.createBundlerOptionsMetrics,
    nativeRegistration: run.nativePluginRegistrationMetrics,
    initialization: run.initializationMetrics,
    termination: run.terminationMetrics,
    workerCount,
    expectedPluginKinds: [
      ...(run.auditSources ? ['ordinary-js'] : []),
      'ordinary-js',
      workerCount === 0 ? 'ordinary-js' : 'parallel-placeholder',
    ],
  });
}
