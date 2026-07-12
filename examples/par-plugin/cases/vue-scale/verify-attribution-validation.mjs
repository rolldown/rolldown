import assert from 'node:assert/strict';
import {
  validateBindingModuleInit,
  validateCpuAttributionArithmetic,
  validateGc,
  validateRustWidthInputs,
} from './attribution-validation.mjs';
import { OUTPUT_GOLDEN_FIELDS, PORTABLE_OUTPUT_GOLDEN_FIELDS } from './correctness-evidence.mjs';
import { validateOutputAgainstGolden } from './evidence-verifier.mjs';
import { assertLocalExecution } from './provenance.mjs';

assertLocalExecution();
const pool = { ROLLDOWN_WORKER_THREADS: '18', ROLLDOWN_MAX_BLOCKING_THREADS: '4' };
const moduleInit = {
  kind: 'rolldown_binding_module_init_metrics',
  version: 1,
  invocationOrdinal: 1,
  configuredTokioWorkerThreads: 18,
  configuredTokioMaxBlockingThreads: 4,
  runtimeBuildMs: 2,
  customRuntimeRegistrationMs: 0.5,
  totalMs: 2.5,
  threadsStartedAfterBuild: 17,
  threadsStoppedAfterBuild: 0,
  threadsStartedAfterRegistration: 18,
  threadsStoppedAfterRegistration: 0,
  interpretation: 'synthetic validation fixture',
};
validateBindingModuleInit(moduleInit, pool);
rejects(() =>
  validateBindingModuleInit(
    withMutation(moduleInit, (value) => delete value.threadsStartedAfterBuild),
    pool,
  ),
);
rejects(() =>
  validateBindingModuleInit(
    withMutation(moduleInit, (value) => (value.totalMs = 3)),
    pool,
  ),
);

const gc = {
  count: 3,
  durationMs: 6,
  maxDurationMs: 3,
  byKind: {
    1: { kind: 1, count: 2, durationMs: 4, maxDurationMs: 3 },
    4: { kind: 4, count: 1, durationMs: 2, maxDurationMs: 2 },
  },
};
validateGc(gc);
rejects(() => validateGc(withMutation(gc, (value) => (value.byKind = null))));
rejects(() => validateGc(withMutation(gc, (value) => (value.count = 4))));

const snapshot = (processUser, processSystem, mainUser, mainSystem) => ({
  processCpuUsageMicros: { user: processUser, system: processSystem },
  mainThreadCpuUsageMicros: { user: mainUser, system: mainSystem },
});
const capture = (user, system) => ({ ok: true, snapshot: { cpuUsageMicros: { user, system } } });
const cpuAttribution = {
  processCpuDeltaMicros: { user: 100, system: 30 },
  mainThreadCpuDeltaMicros: { user: 10, system: 5 },
  measuredWorkerCpuDeltaMicros: { user: 40, system: 10 },
  measuredWorkerThreadCpuDeltaMicros: { user: 40, system: 10 },
  processMinusWorkerThreadCpuDeltaMicros: { user: 60, system: 20 },
  residualProcessCpuDeltaMicros: { user: 50, system: 15 },
  completeWorkerCoverage: true,
  workerCpuScope: 'synthetic worker scope',
  residualMeaning: 'synthetic residual scope',
};
const cpuArguments = {
  value: cpuAttribution,
  processStart: snapshot(10, 5, 2, 1),
  processEnd: snapshot(110, 35, 12, 6),
  workerStarts: [capture(5, 2), capture(7, 3)],
  workerEnds: [capture(25, 7), capture(27, 8)],
};
validateCpuAttributionArithmetic(cpuArguments);
rejects(() =>
  validateCpuAttributionArithmetic({
    ...cpuArguments,
    value: withMutation(cpuAttribution, (value) => (value.residualProcessCpuDeltaMicros.user = 51)),
  }),
);
rejects(() =>
  validateCpuAttributionArithmetic({
    ...cpuArguments,
    value: withMutation(cpuAttribution, (value) => (value.measuredWorkerCpuDeltaMicros.user = 39)),
  }),
);

const timeline = {
  events: [
    { sequence: 0, callOrdinal: 1, phase: 'arrival', atNs: 10, workerIndex: null },
    { sequence: 1, callOrdinal: 1, phase: 'acquire', atNs: 20, workerIndex: 0 },
    { sequence: 2, callOrdinal: 1, phase: 'complete', atNs: 50, workerIndex: 0 },
  ],
  activityEndNs: 50,
  timeWeightedWidths: {
    observationNs: 40,
    pendingWidthNs: 10,
    outstandingWidthNs: 40,
    inFlightWidthNs: 30,
  },
  completionRateInputs: {
    completedCalls: 1,
    activitySpanNs: 40,
    firstCompletionNs: 50,
    lastCompletionNs: 50,
    completionSpanNs: 0,
  },
  workerServiceNs: [
    { workerIndex: 0, completedCalls: 1, total: 31, min: 31, p50: 31, p95: 31, max: 31 },
  ],
};
validateRustWidthInputs(timeline, [50], [[30]]);
rejects(() =>
  validateRustWidthInputs(
    withMutation(timeline, (value) => (value.timeWeightedWidths.pendingWidthNs = 11)),
    [50],
    [[30]],
  ),
);
rejects(() =>
  validateRustWidthInputs(
    withMutation(timeline, (value) => (value.completionRateInputs.completionSpanNs = 1)),
    [50],
    [[30]],
  ),
);
rejects(() =>
  validateRustWidthInputs(
    withMutation(timeline, (value) => (value.workerServiceNs[0].completedCalls = 2)),
    [50],
    [[30]],
  ),
);
rejects(() =>
  validateRustWidthInputs(
    withMutation(timeline, (value) => (value.timeWeightedWidths.unexpected = 0)),
    [50],
    [[30]],
  ),
);

const output = {
  componentCount: 32,
  variant: 'worker-4',
  outputRawCodeHash: '1'.repeat(64),
  outputCodeHash: '2'.repeat(64),
  outputRawMapHash: '3'.repeat(64),
  outputMapHash: '4'.repeat(64),
  outputCodeBytes: 100,
  outputMapBytes: 50,
  outputChunkCount: 1,
  outputAssetCount: 0,
  totalExports: 32,
};
const golden = {
  output: Object.fromEntries(OUTPUT_GOLDEN_FIELDS.map((field) => [field, output[field]])),
};
validateOutputAgainstGolden(output, golden);
for (const field of PORTABLE_OUTPUT_GOLDEN_FIELDS) {
  rejects(() =>
    validateOutputAgainstGolden(
      withMutation(output, (value) => {
        value[field] = typeof value[field] === 'number' ? value[field] + 1 : '0'.repeat(64);
      }),
      golden,
    ),
  );
}

console.log(
  `Vue evidence and attribution validation passed ${10 + PORTABLE_OUTPUT_GOLDEN_FIELDS.length} negative cases`,
);

function withMutation(value, mutate) {
  const copy = structuredClone(value);
  mutate(copy);
  return copy;
}

function rejects(callback) {
  assert.throws(callback);
}
