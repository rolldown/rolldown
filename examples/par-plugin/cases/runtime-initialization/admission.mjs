import { ATTRIBUTION_PACKAGE_ENVIRONMENT, ATTRIBUTION_RUNTIME } from './provenance.mjs';

export const FORMAL_INITIALIZATION_CASES = Object.freeze(
  [
    ['bare-worker', 'none', 'empty'],
    ['retained-runtime-worker', 'binding', 'empty'],
    ['binding-worker-import', 'binding', 'binding'],
    ['retained-package-worker', 'package', 'empty'],
    ['package-worker-import', 'package', 'package'],
  ].map(([name, parentPreload, mode]) =>
    Object.freeze({ name, parentPreload, mode, workerCounts: Object.freeze([1, 2, 4, 8]) }),
  ),
);

export const INITIALIZATION_TIMEOUTS = Object.freeze({
  workerReadyMs: 60_000,
  workerSnapshotMs: 15_000,
  workerTerminationMs: 15_000,
  childProcessMs: 120_000,
});

export const INITIALIZATION_WORKER_SOURCE_PATH =
  'examples/par-plugin/cases/runtime-initialization/worker.mjs';

const CLOCK_ORIGIN_TOLERANCE_MS = 0.001;

const SMOKE_INITIALIZATION_CASES = Object.freeze([
  Object.freeze({ name: 'bare-worker', parentPreload: 'none', mode: 'empty', workerCounts: [1] }),
  Object.freeze({
    name: 'binding-worker-import',
    parentPreload: 'binding',
    mode: 'binding',
    workerCounts: [1, 4],
  }),
  Object.freeze({
    name: 'package-worker-import',
    parentPreload: 'package',
    mode: 'package',
    workerCounts: [1],
  }),
]);

const FORMAL_HOST_POLICY = Object.freeze({
  maximumUptimeSeconds: 86_400,
  maximumStartingSwapBytes: 512 * 1024 ** 2,
  maximumOneMinuteLoadAverage: 2,
  maximumSummedProcessCpuPercentage: 150,
  minimumMemoryFreePercentage: 50,
  requiredPagingDelta: 0,
});

const POST_HOST_POLICY = Object.freeze({
  requiredAcPower: true,
  requiredLowPowerMode: 0,
  requiredNoRecordedThermalWarning: true,
  requiredNoRecordedPerformanceWarning: true,
  maximumUptimeSeconds: 86_400,
  maximumSwapUsedBytes: 512 * 1024 ** 2,
});

export function validateInitializationMatrix(value) {
  if (
    value?.schema !== 1 ||
    !['correctness-smoke', 'formal-attribution'].includes(value.lane) ||
    value.bindingProfile !== 'release' ||
    JSON.stringify(value.configuredPools) !==
      JSON.stringify({ tokio: 18, rayon: 12, blocking: 4 }) ||
    value.runtime?.sourceCommit !== ATTRIBUTION_RUNTIME.sourceCommit ||
    value.runtime?.bindingSha256 !== ATTRIBUTION_RUNTIME.bindingSha256 ||
    value.runtime?.distributionSha256 !== ATTRIBUTION_RUNTIME.distributionSha256 ||
    value.runtime?.distributionBytes !== ATTRIBUTION_RUNTIME.distributionBytes ||
    value.runtime?.packageEntrySha256 !== ATTRIBUTION_RUNTIME.packageEntrySha256 ||
    value.runtime?.packageEntryBytes !== ATTRIBUTION_RUNTIME.packageEntryBytes
  ) {
    throw new Error('initialization matrix header differs from the attribution runtime');
  }
  const expected =
    value.lane === 'formal-attribution' ? FORMAL_INITIALIZATION_CASES : SMOKE_INITIALIZATION_CASES;
  if (
    value.sampleIntervalMs !== 5 ||
    value.sampleOsThreads !== false ||
    value.repeats !== (value.lane === 'formal-attribution' ? 10 : 1) ||
    !same(value.cases, expected)
  ) {
    throw new Error(
      `${value.lane} must use the frozen controls, worker grid, repeats, and non-perturbing sampler`,
    );
  }
  return value;
}

export function flattenInitializationVariants(matrix) {
  validateInitializationMatrix(matrix);
  return matrix.cases.flatMap((definition) =>
    definition.workerCounts.map((workerCount) => ({ ...definition, workerCount })),
  );
}

export function orderInitializationVariants(variants, block) {
  if (
    !Array.isArray(variants) ||
    variants.length === 0 ||
    !Number.isSafeInteger(block) ||
    block < 0
  ) {
    throw new Error('initialization rotation requires variants and a nonnegative block');
  }
  const offset = Math.floor(block / 2) % variants.length;
  const rotated = [...variants.slice(offset), ...variants.slice(0, offset)];
  return block % 2 === 0 ? rotated : rotated.reverse();
}

export function initializationWorkerSourceSha256(harnessProvenance) {
  const matches = (harnessProvenance?.sourceManifest?.entries ?? []).filter(
    (entry) => entry.path === INITIALIZATION_WORKER_SOURCE_PATH,
  );
  if (
    matches.length !== 1 ||
    matches[0].kind !== 'file' ||
    !/^[a-f0-9]{64}$/.test(matches[0].sha256 ?? '')
  ) {
    throw new Error('initialization harness provenance omits the exact worker source');
  }
  return matches[0].sha256;
}

export function parseMacOsPeakRss(stderr, { required }) {
  if (!required) return undefined;
  const matches = [...String(stderr).matchAll(/^\s*(\d+)\s+maximum resident set size\s*$/gm)];
  if (matches.length !== 1) {
    throw new Error(`expected one child peak RSS record, got ${matches.length}`);
  }
  const value = Number(matches[0][1]);
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new Error('child peak RSS record is outside the safe positive integer range');
  }
  return value;
}

export function validateInitializationCase(
  child,
  options,
  expectedRuntime,
  expectedWorkerSourceSha256,
) {
  if (
    child?.schemaVersion !== 2 ||
    child.kind !== 'rolldown-runtime-initialization-case' ||
    child.measurementClass !==
      'instrumented initialization attribution; elapsed values are not wall benchmark evidence' ||
    !same(child.options, options) ||
    child.runtime?.node !== 'v24.18.0' ||
    child.runtime?.nodeBinary !== process.execPath ||
    child.runtime?.nodeEnv !== 'production' ||
    child.runtime?.bindingSha256 !== expectedRuntime.bindingSha256 ||
    child.runtime?.packageEntrySha256 !== expectedRuntime.packageEntrySha256 ||
    child.runtime?.packageEntryBytes !== expectedRuntime.packageEntryBytes ||
    child.runtime?.workerSourceSha256 !== expectedWorkerSourceSha256 ||
    !same(child.runtime?.configuredPools, { tokio: 18, rayon: 12, blocking: 4 }) ||
    child.runtime?.moduleCompileCache?.enabled !== false
  ) {
    throw new Error('initialization child provenance or options mismatch');
  }
  validateTimeline(child.timeline, options);
  validateResources(child.resources, options, child.timeline);
  for (const ready of child.timeline.ready) {
    const resource = child.resources.workerResourcesAfterAllReady.find(
      ({ workerIndex }) => workerIndex === ready.workerIndex,
    );
    if (!cpuAtLeast(resource.cpuUsageMicros, ready.cpuUsageMicros)) {
      throw new Error(`initialization worker ${ready.workerIndex} CPU snapshots regress`);
    }
  }
  const poolStart = Math.min(
    ...child.timeline.constructors.map(({ constructorStartedAt }) => constructorStartedAt.epochMs),
  );
  const allReady = Math.max(...child.timeline.ready.map(({ receivedAt }) => receivedAt.epochMs));
  if (
    child.resources.processBeforePreload.capturedAt.epochMs <
      child.timeline.processStartedAt.epochMs ||
    child.resources.processBeforeWorkers.capturedAt.epochMs > poolStart ||
    child.resources.processAtAllReady.capturedAt.epochMs < allReady ||
    child.resources.processBeforeTermination.capturedAt.epochMs >
      child.timeline.terminationStartedAt.epochMs ||
    child.resources.processAfterTerminationBeforePostGc.capturedAt.epochMs <
      child.timeline.terminationFinishedAt.epochMs
  ) {
    throw new Error('initialization resource snapshots do not bracket the named lifecycle stages');
  }
  const beforePreload = child.resources.processBeforePreload.capturedAt;
  const beforeWorkers = child.resources.processBeforeWorkers.capturedAt;
  if (
    child.timeline.preload &&
    (!timestampsAtOrAfter(child.timeline.preload.startedAt, beforePreload) ||
      !timestampsAtOrAfter(beforeWorkers, child.timeline.preload.finishedAt))
  ) {
    throw new Error('initialization preload is outside its process resource snapshots');
  }
  for (const local of child.resources.workerLocalSnapshots) {
    const workerOrigin = child.timeline.ready.find(
      ({ workerIndex }) => workerIndex === local.workerIndex,
    ).clock.timeOriginEpochMs;
    assertTimestampOrigin(
      local.capturedAt,
      workerOrigin,
      `worker ${local.workerIndex} local snapshot`,
    );
    if (
      !timestampsAtOrAfter(
        local.capturedAt,
        child.resources.processAfterWorkerResourceCapture.capturedAt,
      ) ||
      !timestampsAtOrAfter(child.resources.processBeforeTermination.capturedAt, local.capturedAt)
    ) {
      throw new Error(
        `initialization worker ${local.workerIndex} snapshot is outside its parent bracket`,
      );
    }
  }
  return child;
}

export function validateModuleInitRecords(records, options) {
  const importsNativeLibrary = options.parentPreload !== 'none' || options.mode !== 'empty';
  if (!Array.isArray(records) || records.length !== (importsNativeLibrary ? 1 : 0)) {
    throw new Error('initialization module-init record count is incorrect');
  }
  for (const record of records) {
    if (
      record.kind !== 'rolldown_binding_module_init_metrics' ||
      record.version !== 1 ||
      record.invocationOrdinal !== 1 ||
      record.configuredTokioWorkerThreads !== 18 ||
      record.configuredTokioMaxBlockingThreads !== 4 ||
      !integerBetween(record.threadsStartedAfterBuild, 0, 18) ||
      record.threadsStoppedAfterBuild !== 0 ||
      !integerBetween(
        record.threadsStartedAfterRegistration,
        record.threadsStartedAfterBuild,
        18,
      ) ||
      record.threadsStoppedAfterRegistration !== 0 ||
      !finiteNonnegative(record.runtimeBuildMs) ||
      !finiteNonnegative(record.customRuntimeRegistrationMs) ||
      !finiteNonnegative(record.totalMs) ||
      Math.abs(record.totalMs - record.runtimeBuildMs - record.customRuntimeRegistrationMs) >
        1e-6 ||
      typeof record.interpretation !== 'string' ||
      record.interpretation.length === 0
    ) {
      throw new Error(`initialization module-init record is incomplete: ${JSON.stringify(record)}`);
    }
  }
  return records;
}

export function validateFormalInitializationReport(report) {
  const matrix = validateInitializationMatrix(report?.matrix);
  if (
    matrix.lane !== 'formal-attribution' ||
    report.schemaVersion !== 1 ||
    report.kind !== 'rolldown-runtime-initialization-matrix' ||
    report.measurementClass !==
      'formal local initialization attribution; instrumented elapsed values are not wall benchmark evidence' ||
    !Number.isFinite(Date.parse(report.createdAt)) ||
    report.host?.platform !== 'darwin' ||
    report.host?.architecture !== 'arm64' ||
    report.host?.logicalCpuCount !== 12 ||
    report.host?.totalMemoryBytes !== 38_654_705_664 ||
    !String(report.host?.cpuModel).includes('Apple M3 Pro') ||
    typeof report.host?.release !== 'string' ||
    report.host.release.length === 0 ||
    !/^[a-f0-9]{40}$/.test(report.harnessProvenance?.worktree?.commit ?? '') ||
    report.harnessProvenance?.worktree?.status !== '' ||
    !Number.isSafeInteger(report.harnessProvenance?.sourceManifest?.files) ||
    report.harnessProvenance.sourceManifest.files < 1 ||
    !Number.isSafeInteger(report.harnessProvenance?.sourceManifest?.bytes) ||
    report.harnessProvenance.sourceManifest.bytes < 1 ||
    !/^[a-f0-9]{64}$/.test(report.harnessProvenance?.sourceManifest?.aggregateSha256 ?? '') ||
    report.runtimeProvenance?.worktree?.status !== '' ||
    report.runtimeProvenance?.worktree?.commit !== ATTRIBUTION_RUNTIME.sourceCommit ||
    report.runtimeProvenance?.binding?.sha256 !== ATTRIBUTION_RUNTIME.bindingSha256 ||
    report.runtimeProvenance?.distribution?.aggregateSha256 !==
      ATTRIBUTION_RUNTIME.distributionSha256 ||
    report.runtimeProvenance?.distribution?.bytes !== ATTRIBUTION_RUNTIME.distributionBytes ||
    report.runtimeProvenance?.packageEntry?.sha256 !== ATTRIBUTION_RUNTIME.packageEntrySha256 ||
    report.runtimeProvenance?.packageEntry?.bytes !== ATTRIBUTION_RUNTIME.packageEntryBytes ||
    !same(report.runtimeProvenance?.packageEnvironment, ATTRIBUTION_PACKAGE_ENVIRONMENT) ||
    report.runtimeProvenance?.node?.version !== 'v24.18.0' ||
    report.runtimeProvenance?.node?.path !== process.execPath ||
    !Number.isSafeInteger(report.runtimeProvenance?.node?.bytes) ||
    report.runtimeProvenance.node.bytes < 1 ||
    !/^[a-f0-9]{64}$/.test(report.runtimeProvenance?.node?.sha256 ?? '') ||
    report.executionEnvironment?.inheritedNodeOptions !== null ||
    report.executionEnvironment?.inheritedNodeCompileCache !== null ||
    report.executionEnvironment?.inheritedNodeCompileCachePortable !== null ||
    report.executionEnvironment?.inheritedNodeDisableCompileCache !== null ||
    report.executionEnvironment?.childNodeEnv !== 'production' ||
    !same(report.executionEnvironment?.childPoolEnvironment, {
      ROLLDOWN_WORKER_THREADS: '18',
      RAYON_NUM_THREADS: '12',
      ROLLDOWN_MAX_BLOCKING_THREADS: '4',
    }) ||
    report.executionEnvironment?.exposeGcByArgument !== true ||
    !same(report.executionEnvironment?.timeouts, INITIALIZATION_TIMEOUTS) ||
    report.executionEnvironment?.rotation !== 'paired-block offset with odd blocks reversed'
  ) {
    throw new Error('formal initialization report provenance is incomplete');
  }

  const variants = flattenInitializationVariants(matrix);
  const workerSourceSha256 = initializationWorkerSourceSha256(report.harnessProvenance);
  const expectedRuns = matrix.repeats * variants.length;
  if (report.runs?.length !== expectedRuns || report.hostAdmissions?.length !== expectedRuns) {
    throw new Error('formal initialization run or host-admission count is incomplete');
  }
  for (let sequence = 0; sequence < expectedRuns; sequence++) {
    const block = Math.floor(sequence / variants.length);
    const position = sequence % variants.length;
    const expected = orderInitializationVariants(variants, block)[position];
    const run = report.runs[sequence];
    if (
      run.sequence !== sequence ||
      run.block !== block ||
      run.name !== expected.name ||
      run.mode !== expected.mode ||
      run.parentPreload !== expected.parentPreload ||
      run.workerCount !== expected.workerCount ||
      !Number.isSafeInteger(run.peakRssBytes) ||
      run.peakRssBytes < 1 ||
      run.pagingDelta?.pageouts !== 0 ||
      run.pagingDelta?.swapouts !== 0
    ) {
      throw new Error(`formal initialization rotation is invalid at sequence ${sequence}`);
    }
    const options = {
      mode: expected.mode,
      workerCount: expected.workerCount,
      parentPreload: expected.parentPreload,
      sampleIntervalMs: 5,
      sampleOsThreads: false,
    };
    validateInitializationCase(run.child, options, ATTRIBUTION_RUNTIME, workerSourceSha256);
    validateModuleInitRecords(run.moduleInit, options);
    validatePeakRss(run);
    validateHostAdmission(run.hostAdmission, false);
    validateHostAdmission(run.postHostAdmission, true);
    const recordedAdmission = report.hostAdmissions[sequence];
    if (
      recordedAdmission.block !== block ||
      recordedAdmission.name !== expected.name ||
      recordedAdmission.workerCount !== expected.workerCount ||
      !same(omit(recordedAdmission, ['block', 'name', 'workerCount']), run.hostAdmission)
    ) {
      throw new Error(`formal initialization host admission is unbound at sequence ${sequence}`);
    }
  }
  return true;
}

function validateTimeline(timeline, options) {
  if (
    !validTimestamp(timeline?.processStartedAt) ||
    !Number.isFinite(timeline?.clock?.timeOriginEpochMs) ||
    !Array.isArray(timeline.constructors) ||
    timeline.constructors.length !== options.workerCount ||
    Object.keys(timeline.online ?? {}).length !== options.workerCount ||
    !Array.isArray(timeline.ready) ||
    timeline.ready.length !== options.workerCount ||
    !validTimestamp(timeline.terminationStartedAt) ||
    !validTimestamp(timeline.terminationFinishedAt) ||
    timeline.terminationFinishedAt.epochMs < timeline.terminationStartedAt.epochMs
  ) {
    throw new Error('initialization child timeline is incomplete');
  }
  const parentOrigin = timeline.clock.timeOriginEpochMs;
  assertTimestampOrigin(timeline.processStartedAt, parentOrigin, 'process start');
  assertTimestampOrigin(timeline.terminationStartedAt, parentOrigin, 'termination start');
  assertTimestampOrigin(timeline.terminationFinishedAt, parentOrigin, 'termination finish');
  if ((options.parentPreload === 'none') !== (timeline.preload === undefined)) {
    throw new Error('initialization parent preload boundary mismatch');
  }
  if (
    timeline.preload &&
    (timeline.preload.mode !== options.parentPreload ||
      !orderedTimestamps(timeline.preload.startedAt, timeline.preload.finishedAt))
  ) {
    throw new Error('initialization preload timeline is invalid');
  }
  if (timeline.preload) {
    assertTimestampOrigin(timeline.preload.startedAt, parentOrigin, 'preload start');
    assertTimestampOrigin(timeline.preload.finishedAt, parentOrigin, 'preload finish');
  }
  const constructorIndexes = new Set();
  const readyIndexes = new Set();
  const timeOrigins = new Set();
  for (const constructor of timeline.constructors) {
    if (
      !integerBetween(constructor.workerIndex, 0, options.workerCount - 1) ||
      constructorIndexes.has(constructor.workerIndex) ||
      !orderedTimestamps(constructor.constructorStartedAt, constructor.constructorReturnedAt)
    ) {
      throw new Error('initialization Worker constructor timeline is invalid');
    }
    assertTimestampOrigin(
      constructor.constructorStartedAt,
      parentOrigin,
      `worker ${constructor.workerIndex} constructor start`,
    );
    assertTimestampOrigin(
      constructor.constructorReturnedAt,
      parentOrigin,
      `worker ${constructor.workerIndex} constructor return`,
    );
    constructorIndexes.add(constructor.workerIndex);
  }
  for (let index = 1; index < timeline.constructors.length; index++) {
    if (
      !timestampsAtOrAfter(
        timeline.constructors[index].constructorStartedAt,
        timeline.constructors[index - 1].constructorReturnedAt,
      )
    ) {
      throw new Error('initialization Worker constructors are not serially ordered');
    }
  }
  for (let workerIndex = 0; workerIndex < options.workerCount; workerIndex++) {
    const constructor = timeline.constructors.find((value) => value.workerIndex === workerIndex);
    const online = timeline.online[String(workerIndex)];
    const ready = timeline.ready.find((value) => value.workerIndex === workerIndex);
    if (
      readyIndexes.has(workerIndex) ||
      !constructor ||
      !ready ||
      !validTimestamp(online) ||
      online.epochMs < constructor.constructorReturnedAt.epochMs ||
      !validTimestamp(ready.receivedAt) ||
      !orderedTimestamps(ready.timeline?.entryAt, ready.timeline?.importStartedAt) ||
      !orderedTimestamps(ready.timeline?.importStartedAt, ready.timeline?.importFinishedAt) ||
      ready.timeline.entryAt.epochMs < online.epochMs ||
      ready.receivedAt.epochMs < ready.timeline.importFinishedAt.epochMs ||
      !validCpu(ready.cpuUsageMicros) ||
      !validHeap(ready.heapStatistics) ||
      !validElu(ready.eventLoopUtilization) ||
      !Number.isFinite(ready.clock?.timeOriginEpochMs)
    ) {
      throw new Error(`initialization worker ${workerIndex} timeline is invalid`);
    }
    assertTimestampOrigin(online, parentOrigin, `worker ${workerIndex} online`);
    assertTimestampOrigin(ready.receivedAt, parentOrigin, `worker ${workerIndex} ready receipt`);
    if (Math.abs(ready.clock.timeOriginEpochMs - parentOrigin) > CLOCK_ORIGIN_TOLERANCE_MS) {
      throw new Error(`initialization worker ${workerIndex} clock differs from the parent`);
    }
    for (const [name, value] of Object.entries(ready.timeline)) {
      assertTimestampOrigin(value, ready.clock.timeOriginEpochMs, `worker ${workerIndex} ${name}`);
    }
    readyIndexes.add(workerIndex);
    timeOrigins.add(ready.clock.timeOriginEpochMs);
  }
  if (timeOrigins.size !== 1) throw new Error('initialization worker clocks disagree');
}

function validateResources(resources, options, timeline) {
  const snapshotNames = [
    'processBeforePreload',
    'processBeforeWorkers',
    'processAtAllReady',
    'processAfterWorkerResourceCapture',
    'processBeforeTermination',
    'processAfterTerminationBeforePostGc',
    'processAfterPostGc',
  ];
  for (const name of snapshotNames) {
    validateProcessSnapshot(resources?.[name], name);
    assertTimestampOrigin(resources[name].capturedAt, timeline.clock.timeOriginEpochMs, name);
  }
  for (let index = 1; index < snapshotNames.length; index++) {
    const before = resources[snapshotNames[index - 1]];
    const after = resources[snapshotNames[index]];
    if (
      after.capturedAt.epochMs < before.capturedAt.epochMs ||
      !cpuAtLeast(after.cpuUsageMicros, before.cpuUsageMicros) ||
      !cpuAtLeast(after.mainThreadCpuUsageMicros, before.mainThreadCpuUsageMicros)
    ) {
      throw new Error('initialization process snapshots are not monotonic');
    }
  }
  if (
    !Array.isArray(resources.workerResourcesAfterAllReady) ||
    resources.workerResourcesAfterAllReady.length !== options.workerCount ||
    !Array.isArray(resources.workerLocalSnapshots) ||
    resources.workerLocalSnapshots.length !== options.workerCount ||
    !Array.isArray(resources.samples) ||
    resources.samples.length < 2 ||
    !Number.isSafeInteger(resources.peakSampledRssBytes) ||
    resources.peakSampledRssBytes !==
      Math.max(...resources.samples.map((sample) => sample.memoryUsageBytes?.rss ?? -1))
  ) {
    throw new Error('initialization resource coverage is incomplete');
  }
  const resourceIndexes = new Set();
  const localIndexes = new Set();
  let workerCpuAfterAllReady = 0;
  let workerCpuBeforeTermination = 0;
  for (let workerIndex = 0; workerIndex < options.workerCount; workerIndex++) {
    const resource = resources.workerResourcesAfterAllReady.find(
      (value) => value.workerIndex === workerIndex,
    );
    const local = resources.workerLocalSnapshots.find((value) => value.workerIndex === workerIndex);
    if (
      !resource ||
      !local ||
      resourceIndexes.has(resource.workerIndex) ||
      localIndexes.has(local.workerIndex) ||
      local.postGc !== true ||
      !validTimestamp(local.capturedAt) ||
      !validCpu(resource.cpuUsageMicros) ||
      !validCpu(local.cpuUsageMicros) ||
      !cpuAtLeast(local.cpuUsageMicros, resource.cpuUsageMicros) ||
      !validHeap(resource.heapStatistics) ||
      !validHeap(local.heapStatistics) ||
      !validElu(resource.eventLoopUtilization) ||
      !validElu(local.eventLoopUtilization)
    ) {
      throw new Error(`initialization worker ${workerIndex} resources are incomplete`);
    }
    resourceIndexes.add(resource.workerIndex);
    localIndexes.add(local.workerIndex);
    workerCpuAfterAllReady += sumCpu(resource.cpuUsageMicros);
    workerCpuBeforeTermination += sumCpu(local.cpuUsageMicros);
  }
  const exactProcessCpu = subtractCpu(
    resources.processAfterPostGc.cpuUsageMicros,
    resources.processBeforePreload.cpuUsageMicros,
  );
  const exactMainCpu = subtractCpu(
    resources.processAfterPostGc.mainThreadCpuUsageMicros,
    resources.processBeforePreload.mainThreadCpuUsageMicros,
  );
  if (
    !validCpu(resources.processCpuMicros) ||
    !validCpu(resources.mainCpuMicros) ||
    !validCpu(resources.workerCpuAfterAllReadyMicros) ||
    !validCpu(resources.workerCpuBeforeTerminationMicros) ||
    sumCpu(resources.workerCpuAfterAllReadyMicros) !== workerCpuAfterAllReady ||
    sumCpu(resources.workerCpuBeforeTerminationMicros) !== workerCpuBeforeTermination ||
    sumCpu(resources.workerCpuBeforeTerminationMicros) <
      sumCpu(resources.workerCpuAfterAllReadyMicros) ||
    !same(resources.processCpuMicros, exactProcessCpu) ||
    !same(resources.mainCpuMicros, exactMainCpu) ||
    !Number.isSafeInteger(resources.residualCpuMicros) ||
    resources.residualCpuMicros < 0 ||
    resources.residualCpuMicros !==
      sumCpu(resources.processCpuMicros) -
        sumCpu(resources.mainCpuMicros) -
        sumCpu(resources.workerCpuBeforeTerminationMicros)
  ) {
    throw new Error('initialization CPU attribution is incomplete or inconsistent');
  }
  const priorStates = Array.from({ length: options.workerCount }, () => 0);
  let priorSampleEpoch = -Infinity;
  for (const sample of resources.samples) {
    if (
      !validTimestamp(sample.capturedAt) ||
      sample.capturedAt.epochMs < priorSampleEpoch ||
      !Array.isArray(sample.state) ||
      sample.state.length !== options.workerCount ||
      !Number.isSafeInteger(sample.memoryUsageBytes?.rss)
    ) {
      throw new Error('initialization resource sample is incomplete');
    }
    assertTimestampOrigin(sample.capturedAt, timeline.clock.timeOriginEpochMs, 'resource sample');
    priorSampleEpoch = sample.capturedAt.epochMs;
    for (let index = 0; index < sample.state.length; index++) {
      if (!integerBetween(sample.state[index], 0, 2) || sample.state[index] < priorStates[index]) {
        throw new Error('initialization worker state samples are non-monotonic');
      }
      priorStates[index] = sample.state[index];
    }
  }
  if (priorStates.some((value) => value !== 2)) {
    throw new Error('initialization worker state sampling did not reach ready');
  }
}

function validateHostAdmission(value, postChild) {
  if (
    !value ||
    !Number.isFinite(Date.parse(value.admittedAt)) ||
    value.acPower !== true ||
    value.lowPowerMode !== 0 ||
    value.noRecordedThermalWarning !== true ||
    value.noRecordedPerformanceWarning !== true ||
    !finiteNonnegative(value.uptimeSeconds) ||
    value.uptimeSeconds > 86_400 ||
    !finiteNonnegative(value.swapUsedBytes) ||
    value.swapUsedBytes > 512 * 1024 ** 2 ||
    !same(value.policy, postChild ? POST_HOST_POLICY : FORMAL_HOST_POLICY)
  ) {
    throw new Error(`initialization ${postChild ? 'post' : 'pre'}-child host gate is invalid`);
  }
  if (
    !postChild &&
    (!finiteNonnegative(value.oneMinuteLoadAverage) ||
      value.oneMinuteLoadAverage > 2 ||
      !finiteNonnegative(value.summedProcessCpuPercentage) ||
      value.summedProcessCpuPercentage > 150 ||
      !finiteNonnegative(value.memoryFreePercentage) ||
      value.memoryFreePercentage < 50 ||
      !finiteNonnegative(value.waitedMs))
  ) {
    throw new Error('initialization pre-child transient host gate is invalid');
  }
}

function validateProcessSnapshot(value, name) {
  if (
    !validTimestamp(value?.capturedAt) ||
    !validCpu(value?.cpuUsageMicros) ||
    !validCpu(value?.mainThreadCpuUsageMicros) ||
    !Number.isSafeInteger(value?.memoryUsageBytes?.rss) ||
    value.memoryUsageBytes.rss < 1 ||
    !validHeap(value?.mainIsolateHeapStatistics)
  ) {
    throw new Error(`initialization ${name} snapshot is incomplete`);
  }
}

function validTimestamp(value) {
  return (
    Number.isFinite(value?.monotonicMs) &&
    value.monotonicMs >= 0 &&
    Number.isFinite(value?.epochMs) &&
    value.epochMs > 0
  );
}

function orderedTimestamps(before, after) {
  return (
    validTimestamp(before) &&
    validTimestamp(after) &&
    after.monotonicMs >= before.monotonicMs &&
    after.epochMs >= before.epochMs
  );
}

function validCpu(value) {
  return (
    Number.isSafeInteger(value?.user) &&
    value.user >= 0 &&
    Number.isSafeInteger(value?.system) &&
    value.system >= 0
  );
}

function validHeap(value) {
  return Number.isSafeInteger(value?.used_heap_size) && value.used_heap_size >= 0;
}

function validElu(value) {
  return (
    Number.isFinite(value?.idle) &&
    value.idle >= 0 &&
    Number.isFinite(value?.active) &&
    value.active >= 0 &&
    Number.isFinite(value?.utilization) &&
    value.utilization >= 0 &&
    value.utilization <= 1
  );
}

function finiteNonnegative(value) {
  return Number.isFinite(value) && value >= 0;
}

function integerBetween(value, minimum, maximum) {
  return Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}

function sumCpu(value) {
  return value.user + value.system;
}

function subtractCpu(after, before) {
  return { user: after.user - before.user, system: after.system - before.system };
}

function cpuAtLeast(after, before) {
  return after.user >= before.user && after.system >= before.system;
}

function timestampsAtOrAfter(after, before) {
  return after.monotonicMs >= before.monotonicMs && after.epochMs >= before.epochMs;
}

function assertTimestampOrigin(value, origin, label) {
  if (
    !validTimestamp(value) ||
    !Number.isFinite(origin) ||
    Math.abs(value.epochMs - value.monotonicMs - origin) > CLOCK_ORIGIN_TOLERANCE_MS
  ) {
    throw new Error(`initialization ${label} does not match its monotonic clock origin`);
  }
}

function validatePeakRss(run) {
  const resources = run.child.resources;
  const processSnapshotPeak = Math.max(
    ...[
      resources.processBeforePreload,
      resources.processBeforeWorkers,
      resources.processAtAllReady,
      resources.processAfterWorkerResourceCapture,
      resources.processBeforeTermination,
      resources.processAfterTerminationBeforePostGc,
      resources.processAfterPostGc,
    ].map(({ memoryUsageBytes }) => memoryUsageBytes.rss),
  );
  if (run.peakRssBytes < processSnapshotPeak || run.peakRssBytes < resources.peakSampledRssBytes) {
    throw new Error('initialization external peak RSS is below an in-process observation');
  }
}

function omit(value, keys) {
  return Object.fromEntries(Object.entries(value).filter(([key]) => !keys.includes(key)));
}

function same(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}
