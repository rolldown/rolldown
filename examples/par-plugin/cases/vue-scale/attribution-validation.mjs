import nodePath from 'node:path';

const INIT_KEYS = [
  'cpuAttribution',
  'kind',
  'pluginCount',
  'poolInitializationMs',
  'processSnapshots',
  'rssAfterBytes',
  'rssBeforeBytes',
  'rssScope',
  'version',
  'workerCount',
  'workers',
];
const TERMINATION_KEYS = [
  'cpuAttribution',
  'kind',
  'poolTerminationMs',
  'processSnapshots',
  'rssAfterBytes',
  'rssBeforeBytes',
  'rssScope',
  'version',
  'workerCount',
  'workers',
];
const SNAPSHOT_KEYS = [
  'capturedAt',
  'mainEventLoopUtilization',
  'mainIsolateHeapStatistics',
  'mainThreadCpuUsageMicros',
  'processCpuUsageMicros',
  'processMemoryUsageBytes',
  'processResourceUsage',
  'scope',
];
const CPU_ATTRIBUTION_KEYS = [
  'completeWorkerCoverage',
  'mainThreadCpuDeltaMicros',
  'measuredWorkerCpuDeltaMicros',
  'measuredWorkerThreadCpuDeltaMicros',
  'processCpuDeltaMicros',
  'processMinusWorkerThreadCpuDeltaMicros',
  'residualMeaning',
  'residualProcessCpuDeltaMicros',
  'workerCpuScope',
];

export function validateBindingModuleInit(value, baselinePoolEnvironment) {
  assertExactKeys(
    value,
    [
      'configuredTokioMaxBlockingThreads',
      'configuredTokioWorkerThreads',
      'customRuntimeRegistrationMs',
      'interpretation',
      'invocationOrdinal',
      'kind',
      'runtimeBuildMs',
      'threadsStartedAfterBuild',
      'threadsStartedAfterRegistration',
      'threadsStoppedAfterBuild',
      'threadsStoppedAfterRegistration',
      'totalMs',
      'version',
    ],
    'binding module initialization',
  );
  if (
    value.kind !== 'rolldown_binding_module_init_metrics' ||
    value.version !== 1 ||
    value.invocationOrdinal !== 1 ||
    value.configuredTokioWorkerThreads !==
      Number(baselinePoolEnvironment.ROLLDOWN_WORKER_THREADS) ||
    value.configuredTokioMaxBlockingThreads !==
      Number(baselinePoolEnvironment.ROLLDOWN_MAX_BLOCKING_THREADS) ||
    !isSafeNonnegativeInteger(value.threadsStartedAfterBuild) ||
    !isSafeNonnegativeInteger(value.threadsStoppedAfterBuild) ||
    !isSafeNonnegativeInteger(value.threadsStartedAfterRegistration) ||
    !isSafeNonnegativeInteger(value.threadsStoppedAfterRegistration) ||
    value.threadsStartedAfterRegistration < value.threadsStartedAfterBuild ||
    value.threadsStoppedAfterRegistration < value.threadsStoppedAfterBuild ||
    value.threadsStartedAfterRegistration < 1 ||
    value.threadsStartedAfterRegistration >
      Number(baselinePoolEnvironment.ROLLDOWN_WORKER_THREADS) ||
    value.threadsStoppedAfterRegistration !== 0 ||
    !isFiniteNonnegative(value.runtimeBuildMs) ||
    !isFiniteNonnegative(value.customRuntimeRegistrationMs) ||
    !isFiniteNonnegative(value.totalMs) ||
    !approximatelyEqual(
      value.totalMs,
      value.runtimeBuildMs + value.customRuntimeRegistrationMs,
      1e-6,
    ) ||
    typeof value.interpretation !== 'string' ||
    value.interpretation.length === 0
  ) {
    throw new Error(
      `binding module-initialization attribution is incomplete: ${JSON.stringify(value)}`,
    );
  }
}

export function validateAttributionLifecycle(initialization, termination, workerCount) {
  assertExactKeys(initialization, INIT_KEYS, 'parallel-plugin initialization lifecycle');
  assertExactKeys(termination, TERMINATION_KEYS, 'parallel-plugin termination lifecycle');
  if (
    initialization.kind !== 'rolldown_parallel_plugin_init_metrics' ||
    initialization.version !== 1 ||
    initialization.workerCount !== workerCount ||
    initialization.pluginCount !== 1 ||
    !isFiniteNonnegative(initialization.poolInitializationMs) ||
    termination.kind !== 'rolldown_parallel_plugin_termination_metrics' ||
    termination.version !== 1 ||
    termination.workerCount !== workerCount ||
    !isFiniteNonnegative(termination.poolTerminationMs) ||
    initialization.rssScope !== 'whole process; the before/after delta is not worker ownership' ||
    termination.rssScope !== 'whole process; the before/after delta is not worker ownership'
  ) {
    throw new Error('attribution lifecycle header is incomplete');
  }
  const initSnapshots = validateProcessSnapshotSet(initialization.processSnapshots, [
    'scope',
    'beforeWorkerPool',
    'allWorkersReady',
    'resourceBaselineBeforeBuild',
  ]);
  const termSnapshots = validateProcessSnapshotSet(termination.processSnapshots, [
    'scope',
    'resourceBaselineBeforeBuild',
    'beforeWorkerSnapshots',
    'afterWorkerSnapshots',
    'afterTermination',
  ]);
  assertTimestampOrder(
    [
      initSnapshots.beforeWorkerPool.capturedAt,
      initSnapshots.allWorkersReady.capturedAt,
      initSnapshots.resourceBaselineBeforeBuild.capturedAt,
    ],
    'initialization process snapshots',
  );
  assertTimestampOrder(
    [
      termSnapshots.resourceBaselineBeforeBuild.capturedAt,
      termSnapshots.beforeWorkerSnapshots.capturedAt,
      termSnapshots.afterWorkerSnapshots.capturedAt,
      termSnapshots.afterTermination.capturedAt,
    ],
    'termination process snapshots',
  );
  if (
    JSON.stringify(initSnapshots.resourceBaselineBeforeBuild) !==
      JSON.stringify(termSnapshots.resourceBaselineBeforeBuild) ||
    initialization.rssBeforeBytes !== initSnapshots.beforeWorkerPool.processMemoryUsageBytes.rss ||
    initialization.rssAfterBytes !==
      initSnapshots.resourceBaselineBeforeBuild.processMemoryUsageBytes.rss ||
    termination.rssBeforeBytes !==
      termSnapshots.beforeWorkerSnapshots.processMemoryUsageBytes.rss ||
    termination.rssAfterBytes !== termSnapshots.afterTermination.processMemoryUsageBytes.rss
  ) {
    throw new Error('attribution lifecycle process baseline or RSS accounting differs');
  }
  if (
    !Array.isArray(initialization.workers) ||
    initialization.workers.length !== workerCount ||
    !Array.isArray(termination.workers) ||
    termination.workers.length !== workerCount
  ) {
    throw new Error('attribution lifecycle omitted workers');
  }
  for (let index = 0; index < workerCount; index++) {
    validateInitializedWorker(initialization.workers[index], index);
    validateTerminatedWorker(termination.workers[index], index);
    if (
      JSON.stringify(initialization.workers[index].resourcesAtPoolReady) !==
      JSON.stringify(termination.workers[index].resourcesAtPoolReady)
    ) {
      throw new Error(`worker ${index} resource baseline changed between lifecycle reports`);
    }
  }
  validateCpuAttributionArithmetic({
    value: initialization.cpuAttribution,
    processStart: initSnapshots.beforeWorkerPool,
    processEnd: initSnapshots.resourceBaselineBeforeBuild,
    workerStarts: undefined,
    workerEnds: initialization.workers.map(({ resourcesAtPoolReady }) => resourcesAtPoolReady),
  });
  validateCpuAttributionArithmetic({
    value: termination.cpuAttribution,
    processStart: termSnapshots.resourceBaselineBeforeBuild,
    processEnd: termSnapshots.afterWorkerSnapshots,
    workerStarts: termination.workers.map(({ resourcesAtPoolReady }) => resourcesAtPoolReady),
    workerEnds: termination.workers.map(
      ({ resourcesBeforeTermination }) => resourcesBeforeTermination,
    ),
  });
}

function validateInitializedWorker(value, index) {
  assertExactKeys(
    value,
    ['mainReadyMs', 'mainTimeline', 'resourcesAtPoolReady', 'threadNumber', 'workerBootstrap'],
    `initialized worker ${index}`,
  );
  if (value.threadNumber !== index || !isFiniteNonnegative(value.mainReadyMs)) {
    throw new Error('attribution lifecycle worker numbering is not stable');
  }
  assertExactKeys(
    value.mainTimeline,
    ['constructorReturnedAt', 'constructorStartedAt', 'onlineAt', 'readyMessageAt'],
    `worker ${index} main timeline`,
  );
  assertTimestampOrder(
    [
      value.mainTimeline.constructorStartedAt,
      value.mainTimeline.constructorReturnedAt,
      value.mainTimeline.onlineAt,
      value.mainTimeline.readyMessageAt,
    ],
    `worker ${index} main timeline`,
  );
  if (
    !approximatelyEqual(
      value.mainReadyMs,
      value.mainTimeline.readyMessageAt.monotonicMs -
        value.mainTimeline.constructorStartedAt.monotonicMs,
    )
  ) {
    throw new Error(`worker ${index} ready duration arithmetic differs`);
  }
  validateWorkerResourceCapture(value.resourcesAtPoolReady);
  const bootstrap = value.workerBootstrap;
  assertExactKeys(
    bootstrap,
    [
      'clockAlignment',
      'measuredBootstrapMs',
      'plugins',
      'registerPluginsMs',
      'timeline',
      'workerLocalAtReady',
    ],
    `worker ${index} bootstrap`,
  );
  if (
    !isFiniteNonnegative(bootstrap.measuredBootstrapMs) ||
    !isFiniteNonnegative(bootstrap.registerPluginsMs) ||
    !Array.isArray(bootstrap.plugins) ||
    bootstrap.plugins.length !== 1 ||
    !isFiniteNumber(bootstrap.clockAlignment?.workerTimeOriginEpochMs) ||
    !isFiniteNumber(bootstrap.clockAlignment?.mainTimeOriginEpochMs) ||
    !isFiniteNumber(bootstrap.clockAlignment?.workerMinusMainTimeOriginMs) ||
    !approximatelyEqual(
      bootstrap.clockAlignment.workerMinusMainTimeOriginMs,
      bootstrap.clockAlignment.workerTimeOriginEpochMs -
        bootstrap.clockAlignment.mainTimeOriginEpochMs,
    )
  ) {
    throw new Error(`worker ${index} bootstrap metrics are incomplete`);
  }
  const timelineKeys = [
    'bootstrapStartedAt',
    'entryAt',
    'metricsRuntimeFinishedAt',
    'metricsRuntimeStartedAt',
    'readyAt',
    'registerFinishedAt',
    'registerStartedAt',
  ];
  assertExactKeys(bootstrap.timeline, timelineKeys, `worker ${index} bootstrap timeline`);
  const timeline = bootstrap.timeline;
  assertTimestampOrder(
    [
      timeline.entryAt,
      timeline.bootstrapStartedAt,
      timeline.metricsRuntimeStartedAt,
      timeline.metricsRuntimeFinishedAt,
      timeline.registerStartedAt,
      timeline.registerFinishedAt,
      timeline.readyAt,
    ],
    `worker ${index} bootstrap timeline`,
  );
  if (
    !approximatelyEqual(
      bootstrap.measuredBootstrapMs,
      timeline.readyAt.monotonicMs - timeline.bootstrapStartedAt.monotonicMs,
    ) ||
    !approximatelyEqual(
      bootstrap.registerPluginsMs,
      timeline.registerFinishedAt.monotonicMs - timeline.registerStartedAt.monotonicMs,
    )
  ) {
    throw new Error(`worker ${index} bootstrap duration arithmetic differs`);
  }
  const plugin = bootstrap.plugins[0];
  assertExactKeys(
    plugin,
    ['bindingifyMs', 'factoryMs', 'implementationImportMs', 'pluginIndex', 'timeline'],
    `worker ${index} plugin bootstrap`,
  );
  assertExactKeys(
    plugin.timeline,
    [
      'bindingFinishedAt',
      'bindingStartedAt',
      'factoryFinishedAt',
      'factoryStartedAt',
      'importFinishedAt',
      'importStartedAt',
    ],
    `worker ${index} plugin timeline`,
  );
  assertTimestampOrder(
    [
      plugin.timeline.importStartedAt,
      plugin.timeline.importFinishedAt,
      plugin.timeline.factoryStartedAt,
      plugin.timeline.factoryFinishedAt,
      plugin.timeline.bindingStartedAt,
      plugin.timeline.bindingFinishedAt,
    ],
    `worker ${index} plugin timeline`,
  );
  if (
    !isSafeNonnegativeInteger(plugin.pluginIndex) ||
    !approximatelyEqual(
      plugin.implementationImportMs,
      plugin.timeline.importFinishedAt.monotonicMs - plugin.timeline.importStartedAt.monotonicMs,
    ) ||
    !approximatelyEqual(
      plugin.factoryMs,
      plugin.timeline.factoryFinishedAt.monotonicMs - plugin.timeline.factoryStartedAt.monotonicMs,
    ) ||
    !approximatelyEqual(
      plugin.bindingifyMs,
      plugin.timeline.bindingFinishedAt.monotonicMs - plugin.timeline.bindingStartedAt.monotonicMs,
    )
  ) {
    throw new Error(`worker ${index} plugin bootstrap duration arithmetic differs`);
  }
  validateWorkerLocalMetrics(bootstrap.workerLocalAtReady);
}

function validateTerminatedWorker(value, index) {
  assertExactKeys(
    value,
    [
      'resourcesAtPoolReady',
      'resourcesBeforeTermination',
      'threadNumber',
      'workerLocalBeforeTermination',
    ],
    `terminated worker ${index}`,
  );
  if (value.threadNumber !== index) throw new Error('termination worker numbering differs');
  validateWorkerResourceCapture(value.resourcesAtPoolReady);
  validateWorkerResourceCapture(value.resourcesBeforeTermination);
  validateWorkerLocalMetrics(value.workerLocalBeforeTermination);
  if (
    value.resourcesBeforeTermination.snapshot.captureFinishedAt.monotonicMs <
    value.resourcesAtPoolReady.snapshot.captureFinishedAt.monotonicMs
  ) {
    throw new Error(`worker ${index} termination resource clock moved backwards`);
  }
}

function validateProcessSnapshotSet(value, expectedKeys) {
  assertExactKeys(value, expectedKeys, 'process snapshot set');
  if (value.scope !== 'whole process; RSS is not attributed to an isolate or worker') {
    throw new Error('process snapshot set has an unexpected scope');
  }
  for (const key of expectedKeys.filter((key) => key !== 'scope'))
    validateProcessSnapshot(value[key]);
  return value;
}

function validateProcessSnapshot(snapshot) {
  assertExactKeys(snapshot, SNAPSHOT_KEYS, 'process attribution snapshot');
  assertExactKeys(
    snapshot.scope,
    ['cpuUsage', 'eventLoopUtilization', 'heapStatistics', 'memoryUsage'],
    'process attribution scope',
  );
  if (
    !isTimestamp(snapshot.capturedAt) ||
    !isCpuUsage(snapshot.processCpuUsageMicros) ||
    !isCpuUsage(snapshot.mainThreadCpuUsageMicros) ||
    !isFiniteNonnegative(snapshot.processMemoryUsageBytes?.rss) ||
    !isFiniteNonnegative(snapshot.mainIsolateHeapStatistics?.total_heap_size) ||
    !isEventLoopUtilization(snapshot.mainEventLoopUtilization)
  ) {
    throw new Error('process attribution snapshot is incomplete');
  }
}

export function validateCpuAttributionArithmetic({
  value,
  processStart,
  processEnd,
  workerStarts,
  workerEnds,
}) {
  assertExactKeys(value, CPU_ATTRIBUTION_KEYS, 'CPU attribution');
  if (
    value.completeWorkerCoverage !== true ||
    typeof value.workerCpuScope !== 'string' ||
    typeof value.residualMeaning !== 'string' ||
    !Array.isArray(workerEnds) ||
    workerEnds.length < 1 ||
    workerEnds.some((capture) => capture?.ok !== true) ||
    (workerStarts !== undefined &&
      (!Array.isArray(workerStarts) ||
        workerStarts.length !== workerEnds.length ||
        workerStarts.some((capture) => capture?.ok !== true)))
  ) {
    throw new Error('CPU attribution does not cover every worker');
  }
  const expectedProcess = subtractCpu(
    processEnd.processCpuUsageMicros,
    processStart.processCpuUsageMicros,
  );
  const expectedMain = subtractCpu(
    processEnd.mainThreadCpuUsageMicros,
    processStart.mainThreadCpuUsageMicros,
  );
  const expectedWorkers = workerEnds.reduce(
    (total, end, index) =>
      addCpu(
        total,
        workerStarts
          ? subtractCpu(end.snapshot.cpuUsageMicros, workerStarts[index].snapshot.cpuUsageMicros)
          : end.snapshot.cpuUsageMicros,
      ),
    { user: 0, system: 0 },
  );
  const expectedMinusWorkers = subtractCpu(expectedProcess, expectedWorkers);
  const expectedResidual = subtractCpu(expectedMinusWorkers, expectedMain);
  for (const [name, expected] of [
    ['processCpuDeltaMicros', expectedProcess],
    ['mainThreadCpuDeltaMicros', expectedMain],
    ['measuredWorkerCpuDeltaMicros', expectedWorkers],
    ['measuredWorkerThreadCpuDeltaMicros', expectedWorkers],
    ['processMinusWorkerThreadCpuDeltaMicros', expectedMinusWorkers],
    ['residualProcessCpuDeltaMicros', expectedResidual],
  ]) {
    if (!sameCpu(value[name], expected)) {
      throw new Error(`CPU attribution arithmetic differs for ${name}`);
    }
  }
}

function validateWorkerResourceCapture(capture) {
  assertExactKeys(capture, ['ok', 'snapshot'], 'worker resource capture');
  const snapshot = capture.snapshot;
  assertExactKeys(
    snapshot,
    [
      'captureFinishedAt',
      'captureStartedAt',
      'cpuUsageMicros',
      'eventLoopUtilization',
      'heapStatistics',
    ],
    'worker resource snapshot',
  );
  if (
    capture.ok !== true ||
    !isTimestamp(snapshot.captureStartedAt) ||
    !isTimestamp(snapshot.captureFinishedAt) ||
    snapshot.captureFinishedAt.monotonicMs < snapshot.captureStartedAt.monotonicMs ||
    !isCpuUsage(snapshot.cpuUsageMicros) ||
    !isFiniteNonnegative(snapshot.heapStatistics?.total_heap_size) ||
    !isEventLoopUtilization(snapshot.eventLoopUtilization)
  ) {
    throw new Error('worker CPU/heap/ELU snapshot is incomplete');
  }
}

export function validateWorkerLocalMetrics(value) {
  assertExactKeys(
    value,
    ['capturedAt', 'eventLoopUtilization', 'gc', 'heapStatistics', 'scope'],
    'worker-local metrics',
  );
  if (
    !isTimestamp(value.capturedAt) ||
    !isFiniteNonnegative(value.heapStatistics?.total_heap_size) ||
    !isEventLoopUtilization(value.eventLoopUtilization)
  ) {
    throw new Error('worker-local heap/ELU/GC snapshot is incomplete');
  }
  validateGc(value.gc);
}

export function validateGc(gc) {
  assertExactKeys(gc, ['byKind', 'count', 'durationMs', 'maxDurationMs'], 'worker-local GC');
  if (
    !isSafeNonnegativeInteger(gc.count) ||
    !isFiniteNonnegative(gc.durationMs) ||
    !isFiniteNonnegative(gc.maxDurationMs) ||
    gc.byKind === null ||
    typeof gc.byKind !== 'object' ||
    Array.isArray(gc.byKind)
  ) {
    throw new Error('worker-local GC summary is incomplete');
  }
  const entries = Object.entries(gc.byKind);
  let count = 0;
  let durationMs = 0;
  let maxDurationMs = 0;
  for (const [key, entry] of entries) {
    assertExactKeys(entry, ['count', 'durationMs', 'kind', 'maxDurationMs'], `GC kind ${key}`);
    if (
      String(entry.kind) !== key ||
      !isSafeNonnegativeInteger(entry.kind) ||
      !isSafeNonnegativeInteger(entry.count) ||
      entry.count < 1 ||
      !isFiniteNonnegative(entry.durationMs) ||
      !isFiniteNonnegative(entry.maxDurationMs) ||
      entry.maxDurationMs > entry.durationMs
    ) {
      throw new Error(`worker-local GC kind ${key} is invalid`);
    }
    count += entry.count;
    durationMs += entry.durationMs;
    maxDurationMs = Math.max(maxDurationMs, entry.maxDurationMs);
  }
  if (
    count !== gc.count ||
    !approximatelyEqual(durationMs, gc.durationMs) ||
    !approximatelyEqual(maxDurationMs, gc.maxDurationMs) ||
    (gc.count === 0) !== (entries.length === 0)
  ) {
    throw new Error('worker-local GC byKind totals differ from the summary');
  }
}

export function validateRustTimeline(rust, run, options, workerCount) {
  const timeline = rust.timeline;
  assertExactKeys(
    timeline,
    [
      'activityEndNs',
      'calls',
      'clock',
      'completionRateInputs',
      'events',
      'reportedAtNs',
      'timeWeightedWidths',
      ...(Object.hasOwn(timeline, 'workerIndexToThreadNumber')
        ? ['workerIndexToThreadNumber']
        : []),
      'workerServiceNs',
    ],
    'Rust transform timeline',
  );
  validateRustClock(timeline.clock, timeline.reportedAtNs);
  if (
    !isSafeNonnegativeInteger(timeline.reportedAtNs) ||
    !isSafeNonnegativeInteger(timeline.activityEndNs) ||
    timeline.activityEndNs > timeline.reportedAtNs ||
    !Array.isArray(timeline.calls) ||
    timeline.calls.length !== rust.wrapperCalls ||
    !Array.isArray(timeline.events) ||
    timeline.events.length !== rust.wrapperCalls * 3 ||
    !Array.isArray(timeline.workerServiceNs) ||
    timeline.workerServiceNs.length !== workerCount
  ) {
    throw new Error('Rust transform attribution timeline is incomplete');
  }
  const calls = new Map();
  for (const [index, call] of timeline.calls.entries()) {
    assertExactKeys(call, ['moduleId', 'ordinal'], `Rust call ${index + 1}`);
    if (
      call.ordinal !== index + 1 ||
      typeof call.moduleId !== 'string' ||
      calls.has(call.moduleId)
    ) {
      throw new Error('Rust transform call ordinals or IDs are invalid');
    }
    calls.set(call.moduleId, call.ordinal);
  }
  const eventsByCall = new Map();
  let previousAtNs = -1;
  for (const [sequence, event] of timeline.events.entries()) {
    assertExactKeys(
      event,
      ['atNs', 'callOrdinal', 'phase', 'sequence', 'workerIndex'],
      `Rust event ${sequence}`,
    );
    if (
      event.sequence !== sequence ||
      !isSafeNonnegativeInteger(event.callOrdinal) ||
      !isSafeNonnegativeInteger(event.atNs) ||
      event.atNs < previousAtNs
    ) {
      throw new Error('Rust transform event sequence is invalid');
    }
    previousAtNs = event.atNs;
    const events = eventsByCall.get(event.callOrdinal) ?? [];
    events.push(event);
    eventsByCall.set(event.callOrdinal, events);
  }
  const jsBySourceKey = new Map(
    run.transformTimeline.records.map((record) => [record.sourceKey, record]),
  );
  const rustToJsWorker = new Map();
  const jsToRustWorker = new Map();
  const serviceSamples = Array.from({ length: workerCount }, () => []);
  const completionTimes = [];
  for (const [moduleId, ordinal] of calls) {
    const events = eventsByCall.get(ordinal);
    if (
      events?.length !== 3 ||
      events[0].phase !== 'arrival' ||
      events[1].phase !== 'acquire' ||
      events[2].phase !== 'complete' ||
      events[0].workerIndex !== null ||
      events[1].workerIndex !== events[2].workerIndex ||
      events[1].workerIndex < 0 ||
      events[1].workerIndex >= workerCount ||
      events[1].atNs < events[0].atNs ||
      events[2].atNs < events[1].atNs
    ) {
      throw new Error(`Rust transform phases are incomplete for call ${ordinal}`);
    }
    serviceSamples[events[1].workerIndex].push(events[2].atNs - events[1].atNs);
    completionTimes.push(events[2].atNs);
    if (moduleId.startsWith(`${options._resolvedCorpusDirectory}${nodePath.sep}`)) {
      const sourceKey = moduleId.slice(options._resolvedCorpusDirectory.length + 1);
      const normalizedSourceKey = sourceKey.split(nodePath.sep).join('/');
      const js = jsBySourceKey.get(normalizedSourceKey);
      const rustWorker = events[1].workerIndex;
      if (
        !js ||
        (rustToJsWorker.has(rustWorker) && rustToJsWorker.get(rustWorker) !== js.workerNumber) ||
        (jsToRustWorker.has(js.workerNumber) && jsToRustWorker.get(js.workerNumber) !== rustWorker)
      ) {
        throw new Error(`Rust/JavaScript worker attribution disagrees for ${moduleId}`);
      }
      rustToJsWorker.set(rustWorker, js.workerNumber);
      jsToRustWorker.set(js.workerNumber, rustWorker);
      jsBySourceKey.delete(normalizedSourceKey);
    }
  }
  if (
    jsBySourceKey.size !== 0 ||
    calls.size - run.componentCount !== 3 ||
    rustToJsWorker.size !== workerCount ||
    jsToRustWorker.size !== workerCount
  ) {
    throw new Error(
      'Rust timeline does not contain exactly the selected Vue IDs plus three misses',
    );
  }
  validateRustWidthInputs(timeline, completionTimes, serviceSamples);
  const workerIndexToThreadNumber = Object.fromEntries(
    [...rustToJsWorker].sort(([left], [right]) => left - right),
  );
  if (
    Object.hasOwn(timeline, 'workerIndexToThreadNumber') &&
    JSON.stringify(timeline.workerIndexToThreadNumber) !== JSON.stringify(workerIndexToThreadNumber)
  ) {
    throw new Error('stored Rust-to-JavaScript worker mapping differs from timeline events');
  }
  timeline.workerIndexToThreadNumber = workerIndexToThreadNumber;
}

function validateRustClock(value, reportedAtNs) {
  assertExactKeys(
    value,
    [
      'interpretation',
      'relativeUnit',
      'reportAlignmentUncertaintyNs',
      'reportedRelativeNs',
      'reportedUnixEpochNs',
      'zeroAlignmentUncertaintyNs',
      'zeroUnixEpochNs',
    ],
    'Rust transform clock',
  );
  if (
    value.relativeUnit !== 'nanoseconds' ||
    value.reportedRelativeNs !== reportedAtNs ||
    !isSafeNonnegativeInteger(value.zeroAlignmentUncertaintyNs) ||
    !isSafeNonnegativeInteger(value.reportAlignmentUncertaintyNs) ||
    !/^\d+$/.test(value.zeroUnixEpochNs) ||
    !/^\d+$/.test(value.reportedUnixEpochNs) ||
    BigInt(value.zeroUnixEpochNs) <= 0n ||
    BigInt(value.reportedUnixEpochNs) <= BigInt(value.zeroUnixEpochNs) ||
    typeof value.interpretation !== 'string' ||
    value.interpretation.length === 0
  ) {
    throw new Error('Rust transform clock attribution is incomplete');
  }
  const expectedReportedUnixNs = BigInt(value.zeroUnixEpochNs) + BigInt(reportedAtNs);
  const alignmentDifferenceNs =
    BigInt(value.reportedUnixEpochNs) > expectedReportedUnixNs
      ? BigInt(value.reportedUnixEpochNs) - expectedReportedUnixNs
      : expectedReportedUnixNs - BigInt(value.reportedUnixEpochNs);
  if (
    alignmentDifferenceNs >
    BigInt(value.zeroAlignmentUncertaintyNs + value.reportAlignmentUncertaintyNs + 1_000_000)
  ) {
    throw new Error('Rust transform clock anchors differ by more than one millisecond');
  }
}

export function validateRustWidthInputs(timeline, completionTimes, serviceSamples) {
  assertExactKeys(
    timeline.timeWeightedWidths,
    ['inFlightWidthNs', 'observationNs', 'outstandingWidthNs', 'pendingWidthNs'],
    'Rust time-weighted widths',
  );
  assertExactKeys(
    timeline.completionRateInputs,
    [
      'activitySpanNs',
      'completedCalls',
      'completionSpanNs',
      'firstCompletionNs',
      'lastCompletionNs',
    ],
    'Rust completion-rate inputs',
  );
  const firstEventNs = timeline.events[0]?.atNs ?? timeline.activityEndNs;
  const widths = { pending: 0, outstanding: 0, inFlight: 0 };
  const areas = { pending: 0, outstanding: 0, inFlight: 0 };
  let previous = firstEventNs;
  for (const event of timeline.events) {
    const elapsed = event.atNs - previous;
    areas.pending += elapsed * widths.pending;
    areas.outstanding += elapsed * widths.outstanding;
    areas.inFlight += elapsed * widths.inFlight;
    previous = event.atNs;
    if (event.phase === 'arrival') {
      widths.pending++;
      widths.outstanding++;
    } else if (event.phase === 'acquire') {
      widths.pending--;
      widths.inFlight++;
    } else if (event.phase === 'complete') {
      widths.inFlight--;
      widths.outstanding--;
    }
    if (widths.pending < 0 || widths.outstanding < 0 || widths.inFlight < 0) {
      throw new Error('Rust timeline width state became negative');
    }
  }
  const trailing = timeline.activityEndNs - previous;
  areas.pending += trailing * widths.pending;
  areas.outstanding += trailing * widths.outstanding;
  areas.inFlight += trailing * widths.inFlight;
  if (
    widths.pending !== 0 ||
    widths.outstanding !== 0 ||
    widths.inFlight !== 0 ||
    timeline.timeWeightedWidths.observationNs !== timeline.activityEndNs - firstEventNs ||
    timeline.timeWeightedWidths.pendingWidthNs !== areas.pending ||
    timeline.timeWeightedWidths.outstandingWidthNs !== areas.outstanding ||
    timeline.timeWeightedWidths.inFlightWidthNs !== areas.inFlight ||
    areas.outstanding !== areas.pending + areas.inFlight
  ) {
    throw new Error('Rust time-weighted width inputs do not reconstruct from events');
  }
  const sortedCompletions = [...completionTimes].sort((left, right) => left - right);
  const firstCompletion = sortedCompletions[0] ?? null;
  const lastCompletion = sortedCompletions.at(-1) ?? null;
  if (
    timeline.completionRateInputs.completedCalls !== sortedCompletions.length ||
    timeline.completionRateInputs.activitySpanNs !== timeline.timeWeightedWidths.observationNs ||
    timeline.completionRateInputs.firstCompletionNs !== firstCompletion ||
    timeline.completionRateInputs.lastCompletionNs !== lastCompletion ||
    timeline.completionRateInputs.completionSpanNs !==
      (firstCompletion === null ? 0 : lastCompletion - firstCompletion)
  ) {
    throw new Error('Rust completion-rate inputs do not reconstruct from events');
  }
  let completed = 0;
  for (const [index, samples] of serviceSamples.entries()) {
    const actual = timeline.workerServiceNs[index];
    assertExactKeys(
      actual,
      ['completedCalls', 'max', 'min', 'p50', 'p95', 'total', 'workerIndex'],
      `Rust worker service ${index}`,
    );
    if (
      actual.workerIndex !== index ||
      actual.completedCalls !== samples.length ||
      !isSafeNonnegativeInteger(actual.total) ||
      !isSafeNonnegativeInteger(actual.min) ||
      !isSafeNonnegativeInteger(actual.p50) ||
      !isSafeNonnegativeInteger(actual.p95) ||
      !isSafeNonnegativeInteger(actual.max) ||
      actual.min > actual.p50 ||
      actual.p50 > actual.p95 ||
      actual.p95 > actual.max ||
      actual.total < actual.min * actual.completedCalls ||
      actual.total > actual.max * actual.completedCalls
    ) {
      throw new Error(`Rust worker ${index} service statistics are internally inconsistent`);
    }
    completed += samples.length;
  }
  if (completed !== completionTimes.length) {
    throw new Error('Rust worker service attribution omitted completed calls');
  }
}

function assertTimestampOrder(values, label) {
  for (const value of values) {
    if (!isTimestamp(value)) throw new Error(`${label} has an invalid timestamp`);
  }
  for (let index = 1; index < values.length; index++) {
    if (
      values[index].monotonicMs < values[index - 1].monotonicMs ||
      values[index].epochMs < values[index - 1].epochMs
    ) {
      throw new Error(`${label} moved backwards`);
    }
  }
}

function assertExactKeys(value, expected, label) {
  if (
    value === null ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    JSON.stringify(Object.keys(value).sort(compareStrings)) !==
      JSON.stringify([...expected].sort(compareStrings))
  ) {
    throw new Error(`${label} schema differs`);
  }
}

function compareStrings(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function isTimestamp(value) {
  return value && isFiniteNumber(value.monotonicMs) && isFiniteNumber(value.epochMs);
}

function isCpuUsage(value) {
  return value && isSafeNonnegativeInteger(value.user) && isSafeNonnegativeInteger(value.system);
}

function isEventLoopUtilization(value) {
  return (
    value &&
    isFiniteNonnegative(value.idle) &&
    isFiniteNonnegative(value.active) &&
    isFiniteNumber(value.utilization) &&
    value.utilization >= 0 &&
    value.utilization <= 1
  );
}

function isFiniteNumber(value) {
  return Number.isFinite(value);
}

function isFiniteNonnegative(value) {
  return Number.isFinite(value) && value >= 0;
}

function isSafeNonnegativeInteger(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function addCpu(left, right) {
  return { user: left.user + right.user, system: left.system + right.system };
}

function subtractCpu(end, start) {
  return { user: end.user - start.user, system: end.system - start.system };
}

function sameCpu(actual, expected) {
  return actual?.user === expected.user && actual?.system === expected.system;
}

function approximatelyEqual(left, right, epsilon = 0.05) {
  return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= epsilon;
}
