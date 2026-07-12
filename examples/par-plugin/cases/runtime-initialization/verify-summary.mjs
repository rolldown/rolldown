import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import {
  FORMAL_INITIALIZATION_CASES,
  flattenInitializationVariants,
  INITIALIZATION_TIMEOUTS,
  INITIALIZATION_WORKER_SOURCE_PATH,
  orderInitializationVariants,
  parseMacOsPeakRss,
  validateFormalInitializationReport,
  validateInitializationMatrix,
} from './admission.mjs';
import {
  assertArtifactUnchanged,
  assertDistinctArtifactPaths,
  writeArtifactAtomically,
} from './artifact-io.mjs';
import { ATTRIBUTION_PACKAGE_ENVIRONMENT, ATTRIBUTION_RUNTIME } from './provenance.mjs';
import {
  bootstrapMedianInterval,
  deriveInitializationRun,
  quantile,
  summarizeInitializationReport,
} from './summary-core.mjs';

const formalMatrix = JSON.parse(
  await readFile(new URL('./formal-matrix.json', import.meta.url), 'utf8'),
);
const smokeMatrix = JSON.parse(
  await readFile(new URL('./smoke-matrix.json', import.meta.url), 'utf8'),
);
assert.equal(validateInitializationMatrix(formalMatrix), formalMatrix);
assert.equal(validateInitializationMatrix(smokeMatrix), smokeMatrix);
assert.equal(quantile([1, 2, 3, 4], 0.5), 2.5);
assert.equal(parseMacOsPeakRss(' 123 maximum resident set size\n', { required: true }), 123);
assert.throws(() =>
  parseMacOsPeakRss(' 123 maximum resident set size\n 456 maximum resident set size\n', {
    required: true,
  }),
);
const formalVariants = flattenInitializationVariants(formalMatrix);
for (const workerCount of [1, 2, 4, 8]) {
  for (const [upper, lower] of [
    ['binding-worker-import', 'retained-runtime-worker'],
    ['package-worker-import', 'retained-package-worker'],
  ]) {
    const upperFirst = Array.from({ length: 10 }, (_, block) =>
      orderInitializationVariants(formalVariants, block),
    ).filter(
      (order) =>
        order.findIndex((value) => value.name === upper && value.workerCount === workerCount) <
        order.findIndex((value) => value.name === lower && value.workerCount === workerCount),
    ).length;
    assert.equal(upperFirst, 5, `${upper}/${lower}/worker-${workerCount} order must balance`);
  }
}
assert.deepEqual(
  bootstrapMedianInterval([1, 2, 3, 4], 'determinism'),
  bootstrapMedianInterval([1, 2, 3, 4], 'determinism'),
);
await assert.rejects(() => assertDistinctArtifactPaths(import.meta.filename, import.meta.filename));
const artifactDirectory = await mkdtemp(
  nodePath.join(tmpdir(), 'runtime-initialization-artifact-'),
);
try {
  const rawPath = nodePath.join(artifactDirectory, 'formal.raw.json');
  const summaryPath = nodePath.join(artifactDirectory, 'formal.summary.json');
  const raw = Buffer.from('{"raw":true}\n');
  await writeFile(rawPath, raw);
  await assertDistinctArtifactPaths(rawPath, summaryPath);
  await assertArtifactUnchanged(rawPath, raw);
  await assert.rejects(() => writeArtifactAtomically(rawPath, undefined));
  await assertArtifactUnchanged(rawPath, raw);
  await writeArtifactAtomically(summaryPath, '{"summary":true}\n');
  assert.equal(await readFile(summaryPath, 'utf8'), '{"summary":true}\n');
  await writeFile(rawPath, '{"raw":false}\n');
  await assert.rejects(() => assertArtifactUnchanged(rawPath, raw));
} finally {
  await rm(artifactDirectory, { recursive: true, force: true });
}

for (const mutate of [
  (value) => (value.repeats = 9),
  (value) => (value.sampleIntervalMs = 10),
  (value) => (value.sampleOsThreads = true),
  (value) => (value.runtime.distributionBytes += 1),
  (value) => (value.runtime.packageEntryBytes += 1),
  (value) => value.cases.pop(),
  (value) => value.cases[0].workerCounts.pop(),
]) {
  const invalid = structuredClone(formalMatrix);
  mutate(invalid);
  assert.throws(() => validateInitializationMatrix(invalid));
}

const report = createFormalReport();
assert.equal(validateFormalInitializationReport(report), true);
const summary = summarizeInitializationReport(report, {
  rawArtifact: { path: '/evidence/formal.raw.json', bytes: 123, sha256: 'f'.repeat(64) },
});
assert.equal(summary.cases.length, 20);
assert.equal(summary.source.rawArtifact.sha256, 'f'.repeat(64));
assert.equal(summary.source.distributionBytes, ATTRIBUTION_RUNTIME.distributionBytes);
assert.equal(summary.source.packageEntryBytes, ATTRIBUTION_RUNTIME.packageEntryBytes);
assert.equal(summary.contrasts.length, 4);
assert.equal(summary.contrasts[0].bindingImportOverRetainedRuntime.pairing, 'same rotated block');
assert.equal(summary.contrasts[0].bindingImportOverRetainedRuntime.samples, 10);
assert.equal(
  summary.contrasts[0].packageLayerOverBindingImport.estimator,
  '(package worker import - retained package worker) - (binding worker import - retained runtime worker)',
);
assert.equal(summary.contrasts[0].packageLayerOverBindingImport.metrics.allReadyMs.median, 0);
assert.equal(summary.moduleInitialization.length, 20);
assert.equal(
  summary.contrasts[0].bindingImportOverRetainedRuntime.metrics.allReadyMs.medianBootstrap95
    .lower <=
    summary.contrasts[0].bindingImportOverRetainedRuntime.metrics.allReadyMs.medianBootstrap95
      .upper,
  true,
);
const firstDerived = deriveInitializationRun(report.runs[0]);
assert.equal(firstDerived.allReadyMs, 23);
assert.equal(firstDerived.poolProcessCpuMs, 0.08);
assert.equal(firstDerived.workerCpuBeforeTerminationMs, 0.006);
assert.throws(() => summarizeInitializationReport(report));

for (const mutate of [
  (value) => (value.matrix.repeats = 9),
  (value) => (value.runs[1].block = 1),
  (value) => (value.runs[0].child.runtime.node = 'v24.17.0'),
  (value) => (value.runs[0].child.timeline.ready[0].receivedAt.epochMs = -1),
  (value) => (value.runs[0].child.resources.workerLocalSnapshots = []),
  (value) => (value.runs[0].postHostAdmission.policy.maximumUptimeSeconds = 1),
  (value) => (value.runtimeProvenance.worktree.commit = '0'.repeat(40)),
  (value) => (value.runtimeProvenance.distribution.bytes += 1),
  (value) => (value.runtimeProvenance.packageEntry.bytes += 1),
  (value) => (value.runtimeProvenance.node.sha256 = 'bad'),
  (value) => (value.runtimeProvenance.packageEnvironment.projectFiles['package.json'] = 'bad'),
  (value) => value.hostAdmissions.pop(),
  (value) => (value.runs[0].peakRssBytes = 1),
  (value) => (value.runs[0].child.runtime.packageEntryBytes += 1),
  (value) => (value.runs[0].child.runtime.workerSourceSha256 = '0'.repeat(64)),
  (value) => (value.runs[0].child.timeline.ready[0].clock.timeOriginEpochMs += 1),
  (value) => {
    const run = value.runs.find(({ child: value }) => value.timeline.preload);
    run.child.timeline.preload.startedAt = timestamp(0.1);
    run.child.timeline.preload.finishedAt = timestamp(0.2);
  },
  (value) => (value.runs[0].child.resources.workerLocalSnapshots[0].capturedAt = timestamp(46)),
  (value) => {
    value.runs.find(
      ({ moduleInit }) => moduleInit.length === 1,
    ).moduleInit[0].threadsStoppedAfterRegistration = 1;
  },
  (value) => {
    const resources = value.runs[0].child.resources;
    resources.processCpuMicros = {
      user: resources.mainCpuMicros.user + resources.workerCpuBeforeTerminationMicros.user - 1,
      system: resources.mainCpuMicros.system + resources.workerCpuBeforeTerminationMicros.system,
    };
    resources.residualCpuMicros = -1;
  },
  (value) => (value.runs[0].hostAdmission.admittedAt = 'invalid'),
]) {
  const invalid = structuredClone(report);
  mutate(invalid);
  assert.throws(() => validateFormalInitializationReport(invalid));
}

console.log('runtime initialization admission and summary tests passed');

function createFormalReport() {
  const variants = formalVariants;
  const runs = [];
  const hostAdmissions = [];
  let sequence = 0;
  for (let block = 0; block < 10; block++) {
    const order = orderInitializationVariants(variants, block);
    for (let position = 0; position < order.length; position++) {
      const variant = order[position];
      const options = {
        mode: variant.mode,
        workerCount: variant.workerCount,
        parentPreload: variant.parentPreload,
        sampleIntervalMs: 5,
        sampleOsThreads: false,
      };
      const hostAdmission = preHost(block);
      runs.push({
        sequence: sequence++,
        block,
        name: variant.name,
        mode: variant.mode,
        parentPreload: variant.parentPreload,
        workerCount: variant.workerCount,
        hostAdmission,
        postHostAdmission: postHost(block),
        pagingDelta: { pageouts: 0, swapouts: 0 },
        peakRssBytes: 200_000_000,
        child: child(options),
        moduleInit:
          variant.parentPreload === 'none' && variant.mode === 'empty' ? [] : [moduleInitRecord()],
      });
      hostAdmissions.push({
        block,
        name: variant.name,
        workerCount: variant.workerCount,
        ...hostAdmission,
      });
    }
  }
  return {
    schemaVersion: 1,
    kind: 'rolldown-runtime-initialization-matrix',
    measurementClass:
      'formal local initialization attribution; instrumented elapsed values are not wall benchmark evidence',
    createdAt: '2026-07-12T00:00:00.000Z',
    host: {
      platform: 'darwin',
      release: '25.5.0',
      architecture: 'arm64',
      cpuModel: 'Apple M3 Pro',
      logicalCpuCount: 12,
      totalMemoryBytes: 38_654_705_664,
    },
    executionEnvironment: {
      inheritedNodeOptions: null,
      inheritedNodeCompileCache: null,
      inheritedNodeCompileCachePortable: null,
      inheritedNodeDisableCompileCache: null,
      childNodeEnv: 'production',
      childPoolEnvironment: {
        ROLLDOWN_WORKER_THREADS: '18',
        RAYON_NUM_THREADS: '12',
        ROLLDOWN_MAX_BLOCKING_THREADS: '4',
      },
      exposeGcByArgument: true,
      timeouts: structuredClone(INITIALIZATION_TIMEOUTS),
      rotation: 'paired-block offset with odd blocks reversed',
    },
    hostAdmissions,
    harnessProvenance: {
      worktree: { commit: '1'.repeat(40), status: '' },
      sourceManifest: {
        files: 12,
        bytes: 1000,
        aggregateSha256: '2'.repeat(64),
        entries: [
          {
            path: INITIALIZATION_WORKER_SOURCE_PATH,
            kind: 'file',
            bytes: 100,
            sha256: '4'.repeat(64),
          },
        ],
      },
    },
    runtimeProvenance: {
      packageRoot: '/runtime/packages/rolldown',
      worktree: { commit: ATTRIBUTION_RUNTIME.sourceCommit, status: '' },
      binding: { sha256: ATTRIBUTION_RUNTIME.bindingSha256 },
      distribution: {
        bytes: ATTRIBUTION_RUNTIME.distributionBytes,
        aggregateSha256: ATTRIBUTION_RUNTIME.distributionSha256,
      },
      packageEntry: {
        bytes: ATTRIBUTION_RUNTIME.packageEntryBytes,
        sha256: ATTRIBUTION_RUNTIME.packageEntrySha256,
      },
      packageEnvironment: structuredClone(ATTRIBUTION_PACKAGE_ENVIRONMENT),
      node: {
        version: 'v24.18.0',
        path: process.execPath,
        bytes: 1,
        sha256: '3'.repeat(64),
      },
    },
    matrix: structuredClone(formalMatrix),
    runs,
  };
}

function child(options) {
  const count = options.workerCount;
  const cpuAfterReady = { user: 5 * count, system: 0 };
  const cpuBeforeTermination = { user: 6 * count, system: 0 };
  return {
    schemaVersion: 2,
    kind: 'rolldown-runtime-initialization-case',
    measurementClass:
      'instrumented initialization attribution; elapsed values are not wall benchmark evidence',
    options,
    runtime: {
      node: 'v24.18.0',
      nodeBinary: process.execPath,
      nodeEnv: 'production',
      bindingSha256: ATTRIBUTION_RUNTIME.bindingSha256,
      packageEntryBytes: ATTRIBUTION_RUNTIME.packageEntryBytes,
      packageEntrySha256: ATTRIBUTION_RUNTIME.packageEntrySha256,
      workerSourceSha256: '4'.repeat(64),
      configuredPools: { tokio: 18, rayon: 12, blocking: 4 },
      moduleCompileCache: { enabled: false },
    },
    timeline: {
      clock: { timeOriginEpochMs: 1000 },
      processStartedAt: timestamp(0),
      preload:
        options.parentPreload === 'none'
          ? undefined
          : {
              mode: options.parentPreload,
              startedAt: timestamp(1),
              finishedAt: timestamp(4),
            },
      constructors: Array.from({ length: count }, (_, workerIndex) => ({
        workerIndex,
        constructorStartedAt: timestamp(10 + workerIndex / 5),
        constructorReturnedAt: timestamp(10.1 + workerIndex / 5),
      })),
      online: Object.fromEntries(
        Array.from({ length: count }, (_, workerIndex) => [workerIndex, timestamp(20)]),
      ),
      ready: Array.from({ length: count }, (_, workerIndex) => ({
        type: 'ready',
        workerIndex,
        receivedAt: timestamp(33),
        clock: { timeOriginEpochMs: 1000 },
        timeline: {
          entryAt: timestamp(30),
          importStartedAt: timestamp(31),
          importFinishedAt: timestamp(32),
        },
        cpuUsageMicros: { user: 4, system: 0 },
        heapStatistics: heap(),
        eventLoopUtilization: elu(),
      })),
      terminationStartedAt: timestamp(43),
      terminationFinishedAt: timestamp(44),
    },
    resources: {
      processBeforePreload: processSnapshot(1, 0, 100),
      processBeforeWorkers: processSnapshot(5, 10, 110),
      processAtAllReady: processSnapshot(40, 90, 150),
      processAfterWorkerResourceCapture: processSnapshot(41, 100, 151),
      processBeforeTermination: processSnapshot(42, 110, 152),
      processAfterTerminationBeforePostGc: processSnapshot(44, 120, 145),
      processAfterPostGc: processSnapshot(45, 130, 140),
      workerResourcesAfterAllReady: Array.from({ length: count }, (_, workerIndex) => ({
        workerIndex,
        cpuUsageMicros: { user: 5, system: 0 },
        heapStatistics: heap(),
        eventLoopUtilization: elu(),
      })),
      workerLocalSnapshots: Array.from({ length: count }, (_, workerIndex) => ({
        type: 'snapshot',
        workerIndex,
        capturedAt: timestamp(42),
        postGc: true,
        cpuUsageMicros: { user: 6, system: 0 },
        heapStatistics: heap(),
        eventLoopUtilization: elu(),
      })),
      processCpuMicros: { user: 130, system: 0 },
      mainCpuMicros: { user: 13, system: 0 },
      workerCpuAfterAllReadyMicros: cpuAfterReady,
      workerCpuBeforeTerminationMicros: cpuBeforeTermination,
      residualCpuMicros: 117 - 6 * count,
      samples: [
        { capturedAt: timestamp(15), state: Array(count).fill(0), memoryUsageBytes: { rss: 120 } },
        { capturedAt: timestamp(35), state: Array(count).fill(2), memoryUsageBytes: { rss: 150 } },
      ],
      peakSampledRssBytes: 150,
    },
  };
}

function processSnapshot(time, cpu, rss) {
  return {
    capturedAt: timestamp(time),
    cpuUsageMicros: { user: cpu, system: 0 },
    mainThreadCpuUsageMicros: { user: Math.floor(cpu / 10), system: 0 },
    memoryUsageBytes: { rss },
    mainIsolateHeapStatistics: heap(),
  };
}

function moduleInitRecord() {
  return {
    kind: 'rolldown_binding_module_init_metrics',
    version: 1,
    invocationOrdinal: 1,
    configuredTokioWorkerThreads: 18,
    configuredTokioMaxBlockingThreads: 4,
    runtimeBuildMs: 1,
    customRuntimeRegistrationMs: 2,
    totalMs: 3,
    threadsStartedAfterBuild: 17,
    threadsStoppedAfterBuild: 0,
    threadsStartedAfterRegistration: 18,
    threadsStoppedAfterRegistration: 0,
    interpretation: 'synthetic valid record',
  };
}

function preHost(block) {
  return {
    admittedAt: `2026-07-12T00:00:${String(block).padStart(2, '0')}.000Z`,
    acPower: true,
    lowPowerMode: 0,
    noRecordedThermalWarning: true,
    noRecordedPerformanceWarning: true,
    uptimeSeconds: 100,
    swapUsedBytes: 0,
    oneMinuteLoadAverage: 0,
    summedProcessCpuPercentage: 0,
    memoryFreePercentage: 100,
    waitedMs: 0,
    policy: {
      maximumUptimeSeconds: 86_400,
      maximumStartingSwapBytes: 512 * 1024 ** 2,
      maximumOneMinuteLoadAverage: 2,
      maximumSummedProcessCpuPercentage: 150,
      minimumMemoryFreePercentage: 50,
      requiredPagingDelta: 0,
    },
  };
}

function postHost(block) {
  return {
    admittedAt: `2026-07-12T00:01:${String(block).padStart(2, '0')}.000Z`,
    acPower: true,
    lowPowerMode: 0,
    noRecordedThermalWarning: true,
    noRecordedPerformanceWarning: true,
    uptimeSeconds: 101,
    swapUsedBytes: 0,
    policy: {
      requiredAcPower: true,
      requiredLowPowerMode: 0,
      requiredNoRecordedThermalWarning: true,
      requiredNoRecordedPerformanceWarning: true,
      maximumUptimeSeconds: 86_400,
      maximumSwapUsedBytes: 512 * 1024 ** 2,
    },
  };
}

function timestamp(value) {
  return { monotonicMs: value, epochMs: 1000 + value };
}

function heap() {
  return { used_heap_size: 10 };
}

function elu() {
  return { idle: 0, active: 1, utilization: 1 };
}

assert.deepEqual(
  formalMatrix.cases,
  FORMAL_INITIALIZATION_CASES,
  'the committed formal matrix must equal the exported frozen controls',
);
