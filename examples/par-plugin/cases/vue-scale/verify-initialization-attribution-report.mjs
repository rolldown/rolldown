import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  derivePerWorkerTransformServiceWindows,
  validateJsHookTimingAggregates,
} from './attribution-validation.mjs';
import { validateInitializationAttributionBundle } from './initialization-attribution-validation.mjs';
import {
  ATTRIBUTION_DISTRIBUTION_BYTES,
  ATTRIBUTION_DISTRIBUTION_FILES,
  ATTRIBUTION_DISTRIBUTION_SHA256,
  ATTRIBUTION_NATIVE_BINDING_BYTES,
  ATTRIBUTION_NATIVE_BINDING_SHA256,
  ATTRIBUTION_PACKAGE_ENTRY_BYTES,
  ATTRIBUTION_PACKAGE_ENTRY_SHA256,
  ATTRIBUTION_SOURCE_COMMIT,
  assertLocalExecution,
} from './provenance.mjs';

assertLocalExecution();
const reportPath = process.argv[2];
if (!reportPath) throw new Error('expected an attribution contract report path');
const report = JSON.parse(await readFile(reportPath, 'utf8'));
if (
  report.schema !== 1 ||
  report.matrix?.lane !== 'attribution-contract-smoke' ||
  report.measurementClass !== 'untimed attribution contract validation; not performance evidence' ||
  !Array.isArray(report.runs) ||
  report.runs.length !== 2 ||
  report.executionEnvironment?.exposeGcByArgument !== true ||
  JSON.stringify(report.executionEnvironment?.childExecArgv) !==
    JSON.stringify(['--expose-gc'])
) {
  throw new Error('expected one ordinary/worker-4 attribution contract report');
}
validateReportRuntimeProvenance(report);

for (const [mutate, pattern] of [
  [(value) => (value.matrix.runtimePin.sourceCommit = '0'.repeat(40)), /runtime pin differs/],
  [(value) => value.runtime.nativeBinding.bytes++, /native binding provenance/],
  [
    (value) => (value.runtime.rolldownDistribution.aggregateSha256 = '0'.repeat(64)),
    /distribution provenance/,
  ],
  [(value) => (value.runtime.packageEntry.sha256 = '0'.repeat(64)), /package entry provenance/],
  [(value) => (value.executionEnvironment.exposeGcByArgument = false), /launch contract/],
]) {
  const copy = structuredClone(report);
  mutate(copy);
  assert.throws(
    () => {
      validateReportRuntimeProvenance(copy);
      if (
        copy.executionEnvironment?.exposeGcByArgument !== true ||
        JSON.stringify(copy.executionEnvironment?.childExecArgv) !==
          JSON.stringify(['--expose-gc'])
      ) {
        throw new Error('stored attribution launch contract differs');
      }
    },
    pattern,
  );
}

const ordinary = report.runs.find(({ variant }) => variant === 'ordinary');
const worker = report.runs.find(({ variant }) => variant === 'worker-4');
if (!ordinary || !worker) throw new Error('attribution contract report variants differ');
validate(ordinary);
validate(worker);

const negatives = [
  [ordinary, (run) => run.nativePluginRegistrationMetrics.metricsId++, /metricsId differ/],
  [ordinary, (run) => run.jsMetrics.factoryNsTotal++, /factory timing arithmetic/],
  [worker, (run) => run.jsMetrics.factoryCalls--, /factory timing arithmetic/],
  [worker, (run) => (run.jsMetrics.factoryNsMax = run.jsMetrics.factoryNsTotal + 1), /factory timing arithmetic/],
  [worker, (run) => (run.jsMetrics.buildStartNsTotal = 0), /buildStart timing arithmetic/],
  [worker, (run) => (run.jsMetrics.buildStartNsTotal = Number.NaN), /buildStart timing arithmetic/],
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
  [worker, (run) => delete run.postCloseMetrics, /omitted pool lifecycle metrics/],
  [worker, (run) => run.postCloseMetrics.parentGc.executedPasses--, /two available parent GC/],
  [worker, (run) => (run.postCloseMetrics.parentGc.available = false), /two available parent GC/],
  [worker, (run) => run.postCloseMetrics.parentGc.requestedPasses--, /two available parent GC/],
  [
    worker,
    (run) => run.postCloseMetrics.processSnapshots.parentPostGc.captureFinishedAt.monotonicMs--,
    /capture finish|capture bounds|CPU endpoints|clock origin/,
  ],
  [
    worker,
    (run) => run.postCloseMetrics.cpuWindow.processCpuDeltaMicros.user++,
    /CPU window does not bind|CPU deltas/,
  ],
  [
    worker,
    (run) => run.postCloseMetrics.rss.parentPostGcDeltaFromAfterTerminationBytes++,
    /RSS arithmetic/,
  ],
  [
    worker,
    (run) => (run.postCloseMetrics.cpuWindow.measurementClass = 'exact CPU ownership'),
    /endpoint\/ownership claim/,
  ],
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
    (run) => run.initializationMetrics.cpuWindows.outerProcessWindow.captureBounds.start.latestAt.monotonicMs++,
    /capture bounds|exact process snapshot endpoints|clock origin/,
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
    (run) => run.attributionServiceWindows.workers[0].coldCheckpoints.pop(),
    /service windows differ/,
  ],
  [
    worker,
    (run) => (run.attributionServiceWindows.workers[0].steadyLast256.available = true),
    /service windows differ/,
  ],
  [
    worker,
    (run) => {
      run.transformTimeline.records.pop();
      run.attributionServiceWindows = derivePerWorkerTransformServiceWindows(
        run.transformTimeline.records,
        4,
      );
    },
    /raw transform timeline count/,
  ],
  [
    worker,
    (run) =>
      run.initializationMetrics.workers[0].workerBootstrap.plugins[0].resourceWindows.factory
        .deltas.processRssBytes++,
    /resource deltas differ/,
  ],
  [
    worker,
    (run) =>
      (run.initializationMetrics.workers[0].workerBootstrap.plugins[0].resourceWindows.factory.boundaryRefs.before =
        'wrong-boundary'),
    /boundary references differ/,
  ],
  [
    worker,
    (run) =>
      (run.initializationMetrics.workers[0].workerBootstrap.plugins[0].resourceWindows.factory.scope.processRss =
        'factory-owned RSS'),
    /scope or ownership limits/,
  ],
  [
    worker,
    (run) =>
      run.initializationMetrics.workers[0].workerBootstrap.registrationResources.window.deltas
        .workerThreadCpuUsageMicros.user++,
    /resource deltas differ/,
  ],
  [
    worker,
    (run) => run.initializationMetrics.workers[0].workerBootstrap.registrationStage.durationMs++,
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

function validateReportRuntimeProvenance(value) {
  const expectedPin = {
    kind: 'instrumented-research',
    sourceCommit: ATTRIBUTION_SOURCE_COMMIT,
    nativeBindingSha256: ATTRIBUTION_NATIVE_BINDING_SHA256,
    nativeBindingBytes: ATTRIBUTION_NATIVE_BINDING_BYTES,
    distributionSha256: ATTRIBUTION_DISTRIBUTION_SHA256,
    distributionFiles: ATTRIBUTION_DISTRIBUTION_FILES,
    distributionBytes: ATTRIBUTION_DISTRIBUTION_BYTES,
    packageEntrySha256: ATTRIBUTION_PACKAGE_ENTRY_SHA256,
    packageEntryBytes: ATTRIBUTION_PACKAGE_ENTRY_BYTES,
  };
  if (
    !value.runtime ||
    JSON.stringify(value.matrix.runtimePin) !== JSON.stringify(expectedPin) ||
    JSON.stringify(value.runtime.runtimePin) !== JSON.stringify(expectedPin) ||
    value.runtime.repositoryCommit !== ATTRIBUTION_SOURCE_COMMIT ||
    value.runtime.worktreeStatus !== ''
  ) {
    throw new Error('stored attribution runtime pin differs from the frozen runtime');
  }
  if (
    value.runtime.nativeBinding?.bytes !== ATTRIBUTION_NATIVE_BINDING_BYTES ||
    value.runtime.nativeBinding?.sha256 !== ATTRIBUTION_NATIVE_BINDING_SHA256 ||
    value.runtime.nativeBinding?.sourceCommit !== ATTRIBUTION_SOURCE_COMMIT
  ) {
    throw new Error('stored attribution native binding provenance differs');
  }
  if (
    value.runtime.rolldownDistribution?.files !== ATTRIBUTION_DISTRIBUTION_FILES ||
    value.runtime.rolldownDistribution?.bytes !== ATTRIBUTION_DISTRIBUTION_BYTES ||
    value.runtime.rolldownDistribution?.aggregateSha256 !== ATTRIBUTION_DISTRIBUTION_SHA256
  ) {
    throw new Error('stored attribution distribution provenance differs');
  }
  if (
    value.runtime.packageEntry?.path !== 'packages/rolldown/dist/index.mjs' ||
    value.runtime.packageEntry?.bytes !== ATTRIBUTION_PACKAGE_ENTRY_BYTES ||
    value.runtime.packageEntry?.sha256 !== ATTRIBUTION_PACKAGE_ENTRY_SHA256
  ) {
    throw new Error('stored attribution package entry provenance differs');
  }
}

function validate(run) {
  const workerCount = run.variant === 'ordinary' ? 0 : Number(run.variant.slice('worker-'.length));
  const effectiveWorkerCount = Math.max(1, workerCount);
  if (
    run.transformTimeline?.records?.length !== run.componentCount ||
    run.jsMetrics?.handlerCalls !== run.componentCount ||
    !Array.isArray(run.jsMetrics?.perWorkerCalls) ||
    run.jsMetrics.perWorkerCalls.length !== effectiveWorkerCount ||
    run.jsMetrics.perWorkerCalls.some((calls) => !Number.isSafeInteger(calls) || calls < 1) ||
    run.jsMetrics.perWorkerCalls.reduce((total, calls) => total + calls, 0) !==
      run.jsMetrics.handlerCalls
  ) {
    throw new Error('stored raw transform timeline count differs from exact Vue handler coverage');
  }
  validateJsHookTimingAggregates(run.jsMetrics, Math.max(1, workerCount));
  validateInitializationAttributionBundle({
    createBundlerOptions: run.createBundlerOptionsMetrics,
    nativeRegistration: run.nativePluginRegistrationMetrics,
    initialization: run.initializationMetrics,
    termination: run.terminationMetrics,
    postClose: run.postCloseMetrics,
    workerCount,
    expectedPluginKinds: [
      ...(run.auditSources ? ['ordinary-js'] : []),
      'ordinary-js',
      workerCount === 0 ? 'ordinary-js' : 'parallel-placeholder',
    ],
  });
  const serviceWindows = derivePerWorkerTransformServiceWindows(
    run.transformTimeline.records,
    effectiveWorkerCount,
  );
  assert.deepEqual(
    serviceWindows.workers.map(({ completedCalls }) => completedCalls),
    run.jsMetrics.perWorkerCalls,
    'stored raw timeline and JavaScript per-worker calls differ',
  );
  assert.deepEqual(
    run.attributionServiceWindows,
    serviceWindows,
    'stored per-worker transform service windows differ from the raw timeline',
  );
}
