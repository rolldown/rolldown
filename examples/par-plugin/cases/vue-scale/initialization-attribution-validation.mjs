const CREATE_STAGE_NAMES = [
  'metricsRuntimeSetup',
  'normalizeInputPluginOption',
  'normalizeOutputPluginOption',
  'outputOptionsHook',
  'normalizeHookOutputPluginOption',
  'normalizePluginObjects',
  'parallelPoolInitialization',
  'pluginContextConstruction',
  'bindingifyInputOptions',
  'bindingifyOutputOptions',
];

const CREATE_RESOURCE_NAMES = [
  'afterMetricsRuntimeSetupAtCreateBundlerOptionsStart',
  'afterPluginNormalization',
  'afterParallelPoolInitialization',
  'afterInputBindingification',
  'afterOutputBindingification',
  'atCreateBundlerOptionsFinish',
];

const CREATE_RESOURCE_SCOPE =
  'process CPU/RSS cover the whole process; heap and GC cover the main V8 isolate only';
const CREATE_SNAPSHOT_SCOPE = {
  cpuUsage: 'whole process, including JS workers and native threads',
  mainThreadCpuUsage: 'current Node.js thread only',
  memoryUsage: 'whole process; RSS is not assigned to an isolate or worker',
  heapStatistics: 'current V8 isolate only',
  eventLoopUtilization: 'current Node.js event loop only; this is not CPU time',
  gc: 'GC performance entries observed in this isolate after its metrics observer started',
};
const WORKER_LAUNCHER_SCOPE =
  'research-only metrics entry before the dynamic import of the worker runtime graph; that graph statically imports binding.cjs';
const WORKER_LAUNCHER_SNAPSHOT_SCOPE = {
  cpuUsage: 'whole process, including JS workers and native threads',
  mainThreadCpuUsage: 'current Node.js worker thread only',
  memoryUsage: 'whole process; RSS is not assigned to an isolate or worker',
  heapStatistics: 'current worker V8 isolate only',
  eventLoopUtilization: 'current worker event loop only; this is not CPU time',
  gc: 'GC entries observed in this worker after the research metrics observer started',
};
const WORKER_LOCAL_SCOPE = {
  cpuUsage: 'whole process; not this worker',
  threadCpuUsage: 'this Node.js worker thread',
  memoryUsage: 'whole process; RSS is not this worker',
  heapStatistics: 'this worker V8 isolate',
  eventLoopUtilization: 'this worker event loop; this is not CPU time',
  gc: 'GC performance entries observed in this worker after launcher instrumentation started',
};

export function validateInitializationAttributionBundle({
  createBundlerOptions,
  nativeRegistration,
  initialization,
  termination,
  workerCount,
  expectedPluginKinds,
}) {
  const create = validateCreateBundlerOptionsMetrics(createBundlerOptions, expectedPluginKinds);
  const native = validateNativeRegistrationMetrics(
    nativeRegistration,
    expectedPluginKinds.map((kind) => (kind === 'parallel-placeholder' ? 'parallel-js' : kind)),
    workerCount,
  );
  if (native.metricsId !== create.metricsId) {
    throw new Error('createBundlerOptions and native registration metricsId differ');
  }
  if (
    JSON.stringify(native.plugins.map(({ index, kind }) => ({ index, kind }))) !==
    JSON.stringify(
      create.pluginBinding.map(({ pluginIndex, pluginKind }) => ({
        index: pluginIndex,
        kind: pluginKind === 'parallel-placeholder' ? 'parallel-js' : pluginKind,
      })),
    )
  ) {
    throw new Error('JavaScript binding and native materialization plugin indexes differ');
  }

  if (workerCount === 0) {
    if (initialization || termination) {
      throw new Error('ordinary attribution emitted parallel pool lifecycle metrics');
    }
    return;
  }
  if (!initialization || !termination) {
    throw new Error('parallel attribution omitted pool lifecycle metrics');
  }
  const expectedParallelIndexes = expectedPluginKinds.flatMap((kind, index) =>
    kind === 'parallel-placeholder' ? [index] : [],
  );
  validatePoolLifecycle({
    initialization,
    termination,
    workerCount,
    expectedMetricsId: create.metricsId,
    expectedParallelIndexes,
  });
  if (
    native.workerManagerWorkerCount !== workerCount ||
    native.parallelJsPluginCount !== expectedParallelIndexes.length
  ) {
    throw new Error('native WorkerManager count or parallel plugin count differs from the pool');
  }
}

export function validateCreateBundlerOptionsMetrics(value, expectedPluginKinds) {
  exactKeys(
    value,
    [
      'isolationLimits',
      'kind',
      'measurementClass',
      'metricsId',
      'pluginBinding',
      'pluginCounts',
      'resources',
      'stages',
      'timeline',
      'version',
    ],
    'createBundlerOptions metrics',
  );
  if (
    value.kind !== 'rolldown_create_bundler_options_metrics' ||
    value.version !== 1 ||
    !positiveInteger(value.metricsId) ||
    value.measurementClass !==
      'research-only instrumented initialization attribution; elapsed values are not uninstrumented wall evidence'
  ) {
    throw new Error('createBundlerOptions metrics header is invalid');
  }
  exactKeys(
    value.pluginCounts,
    [
      'builtin',
      'inputBeforeOutputOptionsHook',
      'ordinaryJs',
      'outputBeforeOutputOptionsHook',
      'parallelPlaceholders',
    ],
    'createBundlerOptions plugin counts',
  );
  const expectedCounts = countPluginKinds(expectedPluginKinds);
  if (
    value.pluginCounts.inputBeforeOutputOptionsHook !== expectedPluginKinds.length ||
    value.pluginCounts.outputBeforeOutputOptionsHook !== 0 ||
    value.pluginCounts.ordinaryJs !== expectedCounts.ordinaryJs ||
    value.pluginCounts.parallelPlaceholders !== expectedCounts.parallelPlaceholders ||
    value.pluginCounts.builtin !== expectedCounts.builtin
  ) {
    throw new Error('createBundlerOptions plugin counts differ from the Vue plugin list');
  }

  exactKeys(
    value.timeline,
    ['createBundlerOptionsFinishedAt', 'createBundlerOptionsStartedAt'],
    'createBundlerOptions timeline',
  );
  const startedAt = timestamp(value.timeline.createBundlerOptionsStartedAt, 'create start');
  const finishedAt = timestamp(value.timeline.createBundlerOptionsFinishedAt, 'create finish');
  const clockOrigin = origin(startedAt);
  sameOrigin(finishedAt, clockOrigin, 'create finish');
  if (finishedAt.monotonicMs < startedAt.monotonicMs) {
    throw new Error('createBundlerOptions timeline regresses');
  }
  exactKeys(value.stages, CREATE_STAGE_NAMES, 'createBundlerOptions stages');
  let previousFinish = startedAt.monotonicMs;
  const stages = new Map();
  for (const name of CREATE_STAGE_NAMES) {
    const validated = stage(value.stages[name], `createBundlerOptions ${name}`, clockOrigin);
    if (
      validated.startedAt.monotonicMs < previousFinish ||
      validated.finishedAt.monotonicMs > finishedAt.monotonicMs
    ) {
      throw new Error(`createBundlerOptions stage order differs at ${name}`);
    }
    previousFinish = validated.finishedAt.monotonicMs;
    stages.set(name, validated);
  }

  if (
    !Array.isArray(value.pluginBinding) ||
    value.pluginBinding.length !== expectedPluginKinds.length
  ) {
    throw new Error('createBundlerOptions plugin binding entries are incomplete');
  }
  for (const [index, plugin] of value.pluginBinding.entries()) {
    exactKeys(
      plugin,
      ['pluginIndex', 'pluginKind', 'pluginName', 'stage'],
      `createBundlerOptions plugin ${index}`,
    );
    const bindingStage = stage(plugin.stage, `createBundlerOptions plugin ${index}`, clockOrigin);
    const inputBinding = stages.get('bindingifyInputOptions');
    if (
      plugin.pluginIndex !== index ||
      plugin.pluginKind !== expectedPluginKinds[index] ||
      typeof plugin.pluginName !== 'string' ||
      plugin.pluginName.length === 0 ||
      bindingStage.startedAt.monotonicMs < inputBinding.startedAt.monotonicMs ||
      bindingStage.finishedAt.monotonicMs > inputBinding.finishedAt.monotonicMs
    ) {
      throw new Error(`createBundlerOptions plugin ${index} identity or stage is invalid`);
    }
  }

  exactKeys(value.resources, ['scope', ...CREATE_RESOURCE_NAMES], 'createBundlerOptions resources');
  if (value.resources.scope !== CREATE_RESOURCE_SCOPE) {
    throw new Error('createBundlerOptions resource scope or RSS ownership claim is invalid');
  }
  let previousResource = startedAt.monotonicMs;
  const resources = new Map();
  for (const name of CREATE_RESOURCE_NAMES) {
    const snapshot = processMetrics(
      value.resources[name],
      `create resource ${name}`,
      clockOrigin,
      CREATE_SNAPSHOT_SCOPE,
    );
    if (
      snapshot.capturedAt.monotonicMs < previousResource ||
      snapshot.capturedAt.monotonicMs > finishedAt.monotonicMs
    ) {
      throw new Error(`createBundlerOptions resource order differs at ${name}`);
    }
    previousResource = snapshot.capturedAt.monotonicMs;
    resources.set(name, snapshot);
  }
  const brackets = [
    [
      'afterMetricsRuntimeSetupAtCreateBundlerOptionsStart',
      'metricsRuntimeSetup',
      'normalizeInputPluginOption',
    ],
    ['afterPluginNormalization', 'normalizePluginObjects', 'parallelPoolInitialization'],
    ['afterParallelPoolInitialization', 'parallelPoolInitialization', 'pluginContextConstruction'],
    ['afterInputBindingification', 'bindingifyInputOptions', 'bindingifyOutputOptions'],
    ['afterOutputBindingification', 'bindingifyOutputOptions', null],
    ['atCreateBundlerOptionsFinish', 'bindingifyOutputOptions', null],
  ];
  for (const [resourceName, preceding, following] of brackets) {
    const at = resources.get(resourceName).capturedAt.monotonicMs;
    if (
      at < stages.get(preceding).finishedAt.monotonicMs ||
      (following && at > stages.get(following).startedAt.monotonicMs)
    ) {
      throw new Error(`createBundlerOptions resource ${resourceName} is outside adjacent stages`);
    }
  }
  strings(value.isolationLimits, 'createBundlerOptions isolation limits');
  return value;
}

export function validateNativeRegistrationMetrics(value, expectedPluginKinds, workerCount) {
  exactKeys(
    value,
    [
      'boundary',
      'builtinPluginCount',
      'kind',
      'metricsId',
      'nativeNormalizationTotalMs',
      'nativePluginMaterializationMs',
      'ordinaryJsPluginCount',
      'parallelJsPluginCount',
      'parallelRegistryPresent',
      'plugins',
      'scope',
      'stageRelationships',
      'stages',
      'version',
      'workerManagerWorkerCount',
    ],
    'native registration metrics',
  );
  const expectedCounts = countNativeKinds(expectedPluginKinds);
  if (
    value.kind !== 'rolldown_native_plugin_registration_metrics' ||
    value.version !== 1 ||
    !positiveInteger(value.metricsId) ||
    !nonnegative(value.nativeNormalizationTotalMs) ||
    !nonnegative(value.nativePluginMaterializationMs) ||
    value.ordinaryJsPluginCount !== expectedCounts.ordinaryJs ||
    value.parallelJsPluginCount !== expectedCounts.parallelJs ||
    value.builtinPluginCount !== expectedCounts.builtin ||
    value.parallelRegistryPresent !== workerCount > 0 ||
    value.workerManagerWorkerCount !== workerCount ||
    typeof value.boundary !== 'string' ||
    value.boundary.length === 0 ||
    typeof value.scope !== 'string' ||
    value.scope.length === 0
  ) {
    throw new Error('native registration metrics header or counts are invalid');
  }
  exactKeys(
    value.stages,
    [
      'bindingOptionNormalizationMs',
      'pluginMaterializationMs',
      'registryTransferMs',
      'workerManagerConstructionMs',
    ],
    'native registration stages',
  );
  for (const stageValue of Object.values(value.stages)) {
    if (!nonnegative(stageValue)) throw new Error('native registration stage is invalid');
  }
  if (
    value.stages.pluginMaterializationMs !== value.nativePluginMaterializationMs ||
    value.stages.pluginMaterializationMs > value.stages.bindingOptionNormalizationMs ||
    value.stages.registryTransferMs +
      value.stages.workerManagerConstructionMs +
      value.stages.bindingOptionNormalizationMs >
      value.nativeNormalizationTotalMs + 1e-3
  ) {
    throw new Error('native registration stage containment is invalid');
  }
  exactKeys(
    value.stageRelationships,
    [
      'bindingOptionNormalization',
      'pluginMaterialization',
      'registryTransfer',
      'workerManagerConstruction',
    ],
    'native registration stage relationships',
  );
  if (!Array.isArray(value.plugins) || value.plugins.length !== expectedPluginKinds.length) {
    throw new Error('native registration plugin entries are incomplete');
  }
  for (const [index, plugin] of value.plugins.entries()) {
    exactKeys(plugin, ['index', 'kind', 'materializationMs', 'name'], `native plugin ${index}`);
    if (
      plugin.index !== index ||
      plugin.kind !== expectedPluginKinds[index] ||
      typeof plugin.name !== 'string' ||
      plugin.name.length === 0 ||
      !nonnegative(plugin.materializationMs)
    ) {
      throw new Error(`native plugin ${index} is invalid`);
    }
  }
  return value;
}

function validatePoolLifecycle({
  initialization,
  termination,
  workerCount,
  expectedMetricsId,
  expectedParallelIndexes,
}) {
  const init = validateLifecycleRecord(initialization, true, {
    workerCount,
    expectedMetricsId,
    expectedParallelIndexes,
  });
  const term = validateLifecycleRecord(termination, false, {
    workerCount,
    expectedMetricsId,
    expectedParallelIndexes,
  });
  if (
    JSON.stringify(init.snapshots.allWorkersReady) !==
      JSON.stringify(term.snapshots.allWorkersReady) ||
    JSON.stringify(init.snapshots.resourceBaselineBeforeBuild) !==
      JSON.stringify(term.snapshots.resourceBaselineBeforeBuild)
  ) {
    throw new Error('pool lifecycle shared process baselines differ');
  }
  for (let index = 0; index < workerCount; index++) {
    if (
      JSON.stringify(initialization.workers[index].resourcesAtPoolReady) !==
      JSON.stringify(termination.workers[index].resourcesAtPoolReady)
    ) {
      throw new Error(`worker ${index} pool-ready baseline differs at termination`);
    }
  }
}

function validateLifecycleRecord(value, initialization, expected) {
  exactKeys(
    value,
    [
      'cpuWindows',
      'kind',
      'metricsId',
      'parallelPluginIndexes',
      'pluginCount',
      initialization ? 'poolInitializationMs' : 'poolTerminationMs',
      'processSnapshots',
      'rssAfterBytes',
      'rssBeforeBytes',
      'rssScope',
      'version',
      'workerCount',
      'workers',
    ],
    initialization ? 'pool initialization' : 'pool termination',
  );
  if (
    value.kind !==
      (initialization
        ? 'rolldown_parallel_plugin_init_metrics'
        : 'rolldown_parallel_plugin_termination_metrics') ||
    value.version !== 1 ||
    value.metricsId !== expected.expectedMetricsId ||
    value.workerCount !== expected.workerCount ||
    value.pluginCount !== expected.expectedParallelIndexes.length ||
    JSON.stringify(value.parallelPluginIndexes) !==
      JSON.stringify(expected.expectedParallelIndexes) ||
    !nonnegative(initialization ? value.poolInitializationMs : value.poolTerminationMs) ||
    !positive(value.rssBeforeBytes) ||
    !positive(value.rssAfterBytes) ||
    value.rssScope !== 'whole process; the before/after delta is not worker ownership'
  ) {
    throw new Error('pool lifecycle header, identity, or plugin indexes are invalid');
  }
  const snapshotNames = initialization
    ? ['beforeWorkerPool', 'allWorkersReady', 'resourceBaselineBeforeBuild']
    : [
        'allWorkersReady',
        'resourceBaselineBeforeBuild',
        'beforeWorkerSnapshots',
        'afterWorkerSnapshots',
        'afterTermination',
      ];
  exactKeys(value.processSnapshots, ['scope', ...snapshotNames], 'pool process snapshots');
  if (
    value.processSnapshots.scope !== 'whole process; RSS is not attributed to an isolate or worker'
  ) {
    throw new Error('pool process snapshot scope is invalid');
  }
  const snapshots = {};
  let clockOrigin;
  let previous = -Infinity;
  for (const name of snapshotNames) {
    const snapshot = lifecycleProcessSnapshot(
      value.processSnapshots[name],
      `pool snapshot ${name}`,
    );
    clockOrigin ??= origin(snapshot.capturedAt);
    sameOrigin(snapshot.capturedAt, clockOrigin, `pool snapshot ${name}`);
    if (snapshot.capturedAt.monotonicMs < previous) throw new Error('pool snapshots regress');
    previous = snapshot.capturedAt.monotonicMs;
    snapshots[name] = snapshot;
  }
  const rssBefore = initialization ? snapshots.beforeWorkerPool : snapshots.beforeWorkerSnapshots;
  const rssAfter = initialization
    ? snapshots.resourceBaselineBeforeBuild
    : snapshots.afterTermination;
  if (
    value.rssBeforeBytes !== rssBefore.processMemoryUsageBytes.rss ||
    value.rssAfterBytes !== rssAfter.processMemoryUsageBytes.rss
  ) {
    throw new Error('pool lifecycle RSS fields differ from process snapshots');
  }
  validateCpuWindows(
    value.cpuWindows,
    initialization,
    expected.workerCount,
    snapshots,
    clockOrigin,
  );
  if (!Array.isArray(value.workers) || value.workers.length !== expected.workerCount) {
    throw new Error('pool lifecycle worker count differs');
  }
  for (const [index, worker] of value.workers.entries()) {
    if (initialization) {
      validateInitializedWorker(
        worker,
        index,
        expected.expectedMetricsId,
        expected.expectedParallelIndexes,
        snapshots,
        clockOrigin,
      );
    } else {
      validateTerminatedWorker(worker, index, clockOrigin);
    }
  }
  return { snapshots };
}

function validateInitializedWorker(
  worker,
  index,
  metricsId,
  parallelIndexes,
  snapshots,
  mainOrigin,
) {
  exactKeys(
    worker,
    ['mainReadyMs', 'mainTimeline', 'resourcesAtPoolReady', 'threadNumber', 'workerBootstrap'],
    `initialized worker ${index}`,
  );
  if (worker.threadNumber !== index || !nonnegative(worker.mainReadyMs)) {
    throw new Error(`initialized worker ${index} identity is invalid`);
  }
  exactKeys(
    worker.mainTimeline,
    ['constructorReturnedAt', 'constructorStartedAt', 'onlineAt', 'readyMessageAt'],
    `worker ${index} main timeline`,
  );
  const main = orderedTimestamps(
    [
      worker.mainTimeline.constructorStartedAt,
      worker.mainTimeline.constructorReturnedAt,
      worker.mainTimeline.onlineAt,
      worker.mainTimeline.readyMessageAt,
    ],
    `worker ${index} main timeline`,
    mainOrigin,
  );
  if (!approx(worker.mainReadyMs, main[3].monotonicMs - main[0].monotonicMs)) {
    throw new Error(`worker ${index} main ready duration differs`);
  }
  if (
    main[0].monotonicMs < snapshots.beforeWorkerPool.capturedAt.monotonicMs ||
    main[3].monotonicMs > snapshots.allWorkersReady.capturedAt.monotonicMs
  ) {
    throw new Error(`worker ${index} main timeline is outside pool initialization`);
  }
  const resource = workerResourceCapture(
    worker.resourcesAtPoolReady,
    `worker ${index} pool ready`,
    mainOrigin,
  );
  if (
    resource.startedAt.monotonicMs < snapshots.allWorkersReady.capturedAt.monotonicMs ||
    resource.finishedAt.monotonicMs > snapshots.resourceBaselineBeforeBuild.capturedAt.monotonicMs
  ) {
    throw new Error(`worker ${index} pool-ready capture is outside process snapshots`);
  }
  validateWorkerBootstrap(worker.workerBootstrap, index, metricsId, parallelIndexes, main);
}

function validateWorkerBootstrap(value, threadNumber, metricsId, parallelIndexes, mainTimeline) {
  exactKeys(
    value,
    [
      'clockAlignment',
      'isolationLimits',
      'kind',
      'launcher',
      'measuredBootstrapMs',
      'metricsId',
      'plugins',
      'registerPluginsMs',
      'threadNumber',
      'timeline',
      'version',
      'workerLocalAtReady',
      'workerLocalBeforePluginInitialization',
    ],
    `worker ${threadNumber} bootstrap`,
  );
  if (
    value.kind !== 'rolldown_parallel_plugin_worker_bootstrap_metrics' ||
    value.version !== 1 ||
    value.metricsId !== metricsId ||
    value.threadNumber !== threadNumber ||
    !nonnegative(value.measuredBootstrapMs) ||
    !nonnegative(value.registerPluginsMs)
  ) {
    throw new Error(`worker ${threadNumber} bootstrap header is invalid`);
  }
  exactKeys(
    value.clockAlignment,
    ['mainTimeOriginEpochMs', 'workerMinusMainTimeOriginMs', 'workerTimeOriginEpochMs'],
    `worker ${threadNumber} clock alignment`,
  );
  const workerOrigin = value.clockAlignment.workerTimeOriginEpochMs;
  if (
    !positive(workerOrigin) ||
    !positive(value.clockAlignment.mainTimeOriginEpochMs) ||
    !Number.isFinite(value.clockAlignment.workerMinusMainTimeOriginMs) ||
    !approx(
      value.clockAlignment.workerMinusMainTimeOriginMs,
      workerOrigin - value.clockAlignment.mainTimeOriginEpochMs,
    ) ||
    !approx(value.clockAlignment.mainTimeOriginEpochMs, origin(mainTimeline[0]), 1e-3)
  ) {
    throw new Error(`worker ${threadNumber} clock alignment is invalid`);
  }
  const launcher = validateWorkerLauncher(value.launcher, metricsId, workerOrigin, threadNumber);
  exactKeys(
    value.timeline,
    [
      'bootstrapStartedAt',
      'entryAt',
      'launcherEntryAt',
      'readyAt',
      'registerFinishedAt',
      'registerStartedAt',
      'runtimeAndBindingImportFinishedAt',
      'runtimeAndBindingImportStartedAt',
      'runtimeEntryAt',
    ],
    `worker ${threadNumber} bootstrap timeline`,
  );
  const timeline = orderedTimestamps(
    [
      value.timeline.launcherEntryAt,
      value.timeline.runtimeAndBindingImportStartedAt,
      value.timeline.runtimeAndBindingImportFinishedAt,
      value.timeline.runtimeEntryAt,
      value.timeline.bootstrapStartedAt,
      value.timeline.registerStartedAt,
      value.timeline.registerFinishedAt,
      value.timeline.readyAt,
    ],
    `worker ${threadNumber} bootstrap timeline`,
    workerOrigin,
  );
  const entry = timestamp(value.timeline.entryAt, `worker ${threadNumber} entry`);
  sameOrigin(entry, workerOrigin, `worker ${threadNumber} entry`);
  if (
    !sameTimestamp(entry, timeline[0]) ||
    !sameTimestamp(launcher.timeline.launcherEntryAt, timeline[0]) ||
    !sameTimestamp(launcher.timeline.runtimeAndBindingImportStartedAt, timeline[1]) ||
    !sameTimestamp(launcher.timeline.runtimeAndBindingImportFinishedAt, timeline[2]) ||
    !approx(value.measuredBootstrapMs, timeline[6].monotonicMs - timeline[0].monotonicMs) ||
    !approx(value.registerPluginsMs, timeline[6].monotonicMs - timeline[5].monotonicMs) ||
    timeline[0].epochMs < mainTimeline[0].epochMs ||
    timeline[7].epochMs > mainTimeline[3].epochMs
  ) {
    throw new Error(`worker ${threadNumber} launcher/bootstrap correlation is invalid`);
  }

  if (!Array.isArray(value.plugins) || value.plugins.length !== parallelIndexes.length) {
    throw new Error(`worker ${threadNumber} bootstrap plugin count differs`);
  }
  for (const [offset, plugin] of value.plugins.entries()) {
    exactKeys(
      plugin,
      ['bindingifyMs', 'factoryMs', 'implementationImportMs', 'pluginIndex', 'stages', 'timeline'],
      `worker ${threadNumber} plugin ${offset}`,
    );
    if (plugin.pluginIndex !== parallelIndexes[offset]) {
      throw new Error(`worker ${threadNumber} plugin index differs from pool registration`);
    }
    exactKeys(
      plugin.stages,
      ['bindingifyPlugin', 'factory', 'implementationImport'],
      `worker ${threadNumber} plugin stages`,
    );
    const importStage = stage(plugin.stages.implementationImport, 'worker import', workerOrigin);
    const factoryStage = stage(plugin.stages.factory, 'worker factory', workerOrigin);
    const bindingStage = stage(
      plugin.stages.bindingifyPlugin,
      'worker bindingification',
      workerOrigin,
    );
    exactKeys(
      plugin.timeline,
      [
        'bindingFinishedAt',
        'bindingStartedAt',
        'factoryFinishedAt',
        'factoryStartedAt',
        'importFinishedAt',
        'importStartedAt',
      ],
      `worker ${threadNumber} plugin timeline`,
    );
    const pluginTimeline = orderedTimestamps(
      [
        plugin.timeline.importStartedAt,
        plugin.timeline.importFinishedAt,
        plugin.timeline.factoryStartedAt,
        plugin.timeline.factoryFinishedAt,
        plugin.timeline.bindingStartedAt,
        plugin.timeline.bindingFinishedAt,
      ],
      `worker ${threadNumber} plugin timeline`,
      workerOrigin,
    );
    if (
      !sameTimestamp(importStage.startedAt, pluginTimeline[0]) ||
      !sameTimestamp(importStage.finishedAt, pluginTimeline[1]) ||
      !sameTimestamp(factoryStage.startedAt, pluginTimeline[2]) ||
      !sameTimestamp(factoryStage.finishedAt, pluginTimeline[3]) ||
      !sameTimestamp(bindingStage.startedAt, pluginTimeline[4]) ||
      !sameTimestamp(bindingStage.finishedAt, pluginTimeline[5]) ||
      !approx(plugin.implementationImportMs, importStage.durationMs) ||
      !approx(plugin.factoryMs, factoryStage.durationMs) ||
      !approx(plugin.bindingifyMs, bindingStage.durationMs) ||
      pluginTimeline[0].monotonicMs < timeline[4].monotonicMs ||
      pluginTimeline[5].monotonicMs > timeline[5].monotonicMs
    ) {
      throw new Error(`worker ${threadNumber} plugin stage correlation is invalid`);
    }
  }
  const before = workerLocal(
    value.workerLocalBeforePluginInitialization,
    'worker local before',
    workerOrigin,
  );
  const ready = workerLocal(value.workerLocalAtReady, 'worker local ready', workerOrigin);
  if (
    before.capturedAt.monotonicMs < timeline[4].monotonicMs ||
    before.capturedAt.monotonicMs > value.plugins[0].timeline.importStartedAt.monotonicMs ||
    ready.capturedAt.monotonicMs < timeline[6].monotonicMs ||
    ready.capturedAt.monotonicMs > timeline[7].monotonicMs
  ) {
    throw new Error(`worker ${threadNumber} local snapshots are outside bootstrap`);
  }
  strings(value.isolationLimits, `worker ${threadNumber} isolation limits`);
}

function validateWorkerLauncher(value, metricsId, clockOrigin, threadNumber) {
  exactKeys(
    value,
    ['kind', 'metricsId', 'resources', 'scope', 'stages', 'timeline', 'version'],
    `worker ${threadNumber} launcher`,
  );
  if (
    value.kind !== 'rolldown_parallel_plugin_worker_launcher_metrics' ||
    value.version !== 1 ||
    value.metricsId !== metricsId ||
    value.scope !== WORKER_LAUNCHER_SCOPE
  ) {
    throw new Error(`worker ${threadNumber} launcher header is invalid`);
  }
  exactKeys(
    value.timeline,
    [
      'launcherEntryAt',
      'metricsRuntimeImportFinishedAt',
      'metricsRuntimeImportStartedAt',
      'runtimeAndBindingImportFinishedAt',
      'runtimeAndBindingImportStartedAt',
    ],
    `worker ${threadNumber} launcher timeline`,
  );
  const ordered = orderedTimestamps(
    [
      value.timeline.launcherEntryAt,
      value.timeline.metricsRuntimeImportStartedAt,
      value.timeline.metricsRuntimeImportFinishedAt,
      value.timeline.runtimeAndBindingImportStartedAt,
      value.timeline.runtimeAndBindingImportFinishedAt,
    ],
    `worker ${threadNumber} launcher timeline`,
    clockOrigin,
  );
  exactKeys(value.stages, ['metricsRuntimeImport', 'runtimeAndBindingImport'], 'launcher stages');
  const metricsStage = stage(
    value.stages.metricsRuntimeImport,
    'metrics runtime import',
    clockOrigin,
  );
  const runtimeStage = stage(
    value.stages.runtimeAndBindingImport,
    'runtime and binding import',
    clockOrigin,
  );
  if (
    !sameTimestamp(metricsStage.startedAt, ordered[1]) ||
    !sameTimestamp(metricsStage.finishedAt, ordered[2]) ||
    !sameTimestamp(runtimeStage.startedAt, ordered[3]) ||
    !sameTimestamp(runtimeStage.finishedAt, ordered[4])
  ) {
    throw new Error(`worker ${threadNumber} launcher stages differ from timeline`);
  }
  exactKeys(
    value.resources,
    ['afterMetricsRuntimeImportBeforeRuntimeAndBindingImport', 'afterRuntimeAndBindingImport'],
    `worker ${threadNumber} launcher resources`,
  );
  const before = processMetrics(
    value.resources.afterMetricsRuntimeImportBeforeRuntimeAndBindingImport,
    'launcher before runtime',
    clockOrigin,
    WORKER_LAUNCHER_SNAPSHOT_SCOPE,
  );
  const after = processMetrics(
    value.resources.afterRuntimeAndBindingImport,
    'launcher after runtime',
    clockOrigin,
    WORKER_LAUNCHER_SNAPSHOT_SCOPE,
  );
  if (
    before.capturedAt.monotonicMs < ordered[2].monotonicMs ||
    before.capturedAt.monotonicMs > ordered[3].monotonicMs ||
    after.capturedAt.monotonicMs < ordered[4].monotonicMs
  ) {
    throw new Error(`worker ${threadNumber} launcher resources do not bracket imports`);
  }
  return { timeline: value.timeline };
}

function validateTerminatedWorker(worker, index, clockOrigin) {
  exactKeys(
    worker,
    [
      'resourcesAtPoolReady',
      'resourcesBeforeTermination',
      'threadNumber',
      'workerLocalBeforeTermination',
    ],
    `terminated worker ${index}`,
  );
  if (worker.threadNumber !== index) throw new Error(`terminated worker ${index} identity differs`);
  const start = workerResourceCapture(
    worker.resourcesAtPoolReady,
    'pool-ready resource',
    clockOrigin,
  );
  const end = workerResourceCapture(
    worker.resourcesBeforeTermination,
    'termination resource',
    clockOrigin,
  );
  const local = workerLocal(
    worker.workerLocalBeforeTermination,
    'worker local termination',
    origin(localTimestamp(worker.workerLocalBeforeTermination)),
  );
  if (
    end.startedAt.monotonicMs < start.finishedAt.monotonicMs ||
    local.capturedAt.epochMs < end.startedAt.epochMs ||
    local.capturedAt.epochMs > end.finishedAt.epochMs + 1
  ) {
    throw new Error(`terminated worker ${index} resource order differs`);
  }
}

function validateCpuWindows(value, initialization, workerCount, snapshots, clockOrigin) {
  exactKeys(
    value,
    [
      'completeWorkerCoverage',
      ...(initialization ? [] : ['innerProcessWindow']),
      'measurementClass',
      'outerProcessWindow',
      'phase',
      'scope',
      'summedObservedWorkerThreadCpuMicros',
      'workerSamples',
    ],
    'CPU windows',
  );
  if (
    value.measurementClass !== 'asynchronous-bracketing-diagnostic; not exact CPU attribution' ||
    value.completeWorkerCoverage !== true ||
    value.phase !==
      (initialization ? 'initialization' : 'lifetime-through-pre-termination-snapshot') ||
    typeof value.scope !== 'string' ||
    !value.scope.includes('never subtracted into a claimed Rust/native residual')
  ) {
    throw new Error('CPU windows make an invalid attribution claim');
  }
  const outer = cpuProcessWindow(value.outerProcessWindow, 'outer CPU window', clockOrigin);
  const expectedOuterStart = snapshots[initialization ? 'beforeWorkerPool' : 'allWorkersReady'];
  const expectedOuterEnd =
    snapshots[initialization ? 'resourceBaselineBeforeBuild' : 'afterWorkerSnapshots'];
  if (
    !sameTimestamp(outer.startedAt, expectedOuterStart.capturedAt) ||
    !sameTimestamp(outer.finishedAt, expectedOuterEnd.capturedAt) ||
    !sameCpu(
      outer.processCpuDeltaMicros,
      cpuDelta(expectedOuterStart, expectedOuterEnd, 'processCpuUsageMicros'),
    ) ||
    !sameCpu(
      outer.mainThreadCpuDeltaMicros,
      cpuDelta(expectedOuterStart, expectedOuterEnd, 'mainThreadCpuUsageMicros'),
    )
  ) {
    throw new Error('outer CPU window does not match exact process snapshot endpoints');
  }
  let inner;
  if (!initialization) {
    inner = cpuProcessWindow(value.innerProcessWindow, 'inner CPU window', clockOrigin);
    if (
      !sameTimestamp(inner.startedAt, snapshots.resourceBaselineBeforeBuild.capturedAt) ||
      !sameTimestamp(inner.finishedAt, snapshots.beforeWorkerSnapshots.capturedAt) ||
      !sameCpu(
        inner.processCpuDeltaMicros,
        cpuDelta(
          snapshots.resourceBaselineBeforeBuild,
          snapshots.beforeWorkerSnapshots,
          'processCpuUsageMicros',
        ),
      ) ||
      !sameCpu(
        inner.mainThreadCpuDeltaMicros,
        cpuDelta(
          snapshots.resourceBaselineBeforeBuild,
          snapshots.beforeWorkerSnapshots,
          'mainThreadCpuUsageMicros',
        ),
      )
    ) {
      throw new Error('inner CPU window does not match exact process snapshot endpoints');
    }
  }
  if (!Array.isArray(value.workerSamples) || value.workerSamples.length !== workerCount) {
    throw new Error('CPU windows omitted worker samples');
  }
  const sum = { user: 0, system: 0 };
  for (const [index, sample] of value.workerSamples.entries()) {
    exactKeys(
      sample,
      [
        'cpuDeltaMicros',
        'endBounds',
        'measurementClass',
        'ok',
        'relationToProcessWindows',
        'startBounds',
        'threadNumber',
      ],
      `CPU worker sample ${index}`,
    );
    if (
      sample.ok !== true ||
      sample.threadNumber !== index ||
      typeof sample.measurementClass !== 'string' ||
      typeof sample.relationToProcessWindows !== 'string'
    ) {
      throw new Error(`CPU worker sample ${index} header is invalid`);
    }
    const start = bounds(sample.startBounds, `CPU worker ${index} start`, clockOrigin);
    const end = bounds(sample.endBounds, `CPU worker ${index} end`, clockOrigin);
    const cpu = cpuUsage(sample.cpuDeltaMicros, `CPU worker ${index}`);
    if (
      start.earliestAt.monotonicMs < outer.startedAt.monotonicMs ||
      end.latestAt.monotonicMs > outer.finishedAt.monotonicMs ||
      end.earliestAt.monotonicMs < start.latestAt.monotonicMs ||
      (inner &&
        (start.latestAt.monotonicMs > inner.startedAt.monotonicMs ||
          end.earliestAt.monotonicMs < inner.finishedAt.monotonicMs))
    ) {
      throw new Error(`CPU worker sample ${index} is outside its declared bounds`);
    }
    sum.user += cpu.user;
    sum.system += cpu.system;
  }
  if (!sameCpu(value.summedObservedWorkerThreadCpuMicros, sum)) {
    throw new Error('CPU worker sample sum differs');
  }
}

function cpuProcessWindow(value, label, clockOrigin) {
  exactKeys(
    value,
    ['finishedAt', 'mainThreadCpuDeltaMicros', 'processCpuDeltaMicros', 'startedAt'],
    label,
  );
  const startedAt = timestamp(value.startedAt, `${label} start`);
  const finishedAt = timestamp(value.finishedAt, `${label} finish`);
  sameOrigin(startedAt, clockOrigin, `${label} start`);
  sameOrigin(finishedAt, clockOrigin, `${label} finish`);
  if (finishedAt.monotonicMs < startedAt.monotonicMs) throw new Error(`${label} regresses`);
  return {
    startedAt,
    finishedAt,
    processCpuDeltaMicros: cpuUsage(value.processCpuDeltaMicros, `${label} process CPU`),
    mainThreadCpuDeltaMicros: cpuUsage(value.mainThreadCpuDeltaMicros, `${label} main CPU`),
  };
}

function bounds(value, label, clockOrigin) {
  exactKeys(value, ['earliestAt', 'latestAt', 'meaning'], label);
  const earliestAt = timestamp(value.earliestAt, `${label} earliest`);
  const latestAt = timestamp(value.latestAt, `${label} latest`);
  sameOrigin(earliestAt, clockOrigin, `${label} earliest`);
  sameOrigin(latestAt, clockOrigin, `${label} latest`);
  if (
    latestAt.monotonicMs < earliestAt.monotonicMs ||
    typeof value.meaning !== 'string' ||
    value.meaning.length === 0
  ) {
    throw new Error(`${label} is invalid`);
  }
  return { earliestAt, latestAt };
}

function lifecycleProcessSnapshot(value, label) {
  exactKeys(
    value,
    [
      'capturedAt',
      'mainEventLoopUtilization',
      'mainIsolateGc',
      'mainIsolateHeapStatistics',
      'mainThreadCpuUsageMicros',
      'processCpuUsageMicros',
      'processMemoryUsageBytes',
      'processResourceUsage',
      'scope',
    ],
    label,
  );
  timestamp(value.capturedAt, `${label} timestamp`);
  cpuUsage(value.processCpuUsageMicros, `${label} process CPU`);
  cpuUsage(value.mainThreadCpuUsageMicros, `${label} main CPU`);
  positiveRss(value.processMemoryUsageBytes, label);
  positiveHeap(value.mainIsolateHeapStatistics, label);
  eventLoop(value.mainEventLoopUtilization, label);
  gc(value.mainIsolateGc, label);
  return value;
}

function processMetrics(value, label, clockOrigin, expectedScope) {
  exactKeys(
    value,
    [
      'capturedAt',
      'isolateEventLoopUtilization',
      'isolateGc',
      'isolateHeapStatistics',
      'mainThreadCpuUsageMicros',
      'processCpuUsageMicros',
      'processMemoryUsageBytes',
      'processResourceUsage',
      'scope',
    ],
    label,
  );
  exactScope(value.scope, expectedScope, `${label} scope`);
  const capturedAt = timestamp(value.capturedAt, `${label} timestamp`);
  sameOrigin(capturedAt, clockOrigin, `${label} timestamp`);
  cpuUsage(value.processCpuUsageMicros, `${label} process CPU`);
  cpuUsage(value.mainThreadCpuUsageMicros, `${label} thread CPU`);
  positiveRss(value.processMemoryUsageBytes, label);
  positiveHeap(value.isolateHeapStatistics, label);
  eventLoop(value.isolateEventLoopUtilization, label);
  gc(value.isolateGc, label);
  return value;
}

function workerResourceCapture(value, label, clockOrigin) {
  exactKeys(value, ['ok', 'snapshot'], label);
  if (value.ok !== true) throw new Error(`${label} failed`);
  exactKeys(
    value.snapshot,
    [
      'captureFinishedAt',
      'captureStartedAt',
      'cpuUsageMicros',
      'eventLoopUtilization',
      'heapStatistics',
    ],
    `${label} snapshot`,
  );
  const startedAt = timestamp(value.snapshot.captureStartedAt, `${label} start`);
  const finishedAt = timestamp(value.snapshot.captureFinishedAt, `${label} finish`);
  sameOrigin(startedAt, clockOrigin, `${label} start`);
  sameOrigin(finishedAt, clockOrigin, `${label} finish`);
  if (finishedAt.monotonicMs < startedAt.monotonicMs) throw new Error(`${label} regresses`);
  cpuUsage(value.snapshot.cpuUsageMicros, `${label} CPU`);
  positiveHeap(value.snapshot.heapStatistics, label);
  eventLoop(value.snapshot.eventLoopUtilization, label);
  return { startedAt, finishedAt };
}

function workerLocal(value, label, clockOrigin) {
  exactKeys(
    value,
    [
      'capturedAt',
      'eventLoopUtilization',
      'gc',
      'heapStatistics',
      'processCpuUsageMicros',
      'processMemoryUsageBytes',
      'scope',
      'threadCpuUsageMicros',
    ],
    label,
  );
  exactScope(value.scope, WORKER_LOCAL_SCOPE, `${label} scope`);
  const capturedAt = timestamp(value.capturedAt, `${label} timestamp`);
  sameOrigin(capturedAt, clockOrigin, `${label} timestamp`);
  cpuUsage(value.processCpuUsageMicros, `${label} process CPU`);
  cpuUsage(value.threadCpuUsageMicros, `${label} thread CPU`);
  positiveRss(value.processMemoryUsageBytes, label);
  positiveHeap(value.heapStatistics, label);
  eventLoop(value.eventLoopUtilization, label);
  gc(value.gc, label);
  return { capturedAt };
}

function localTimestamp(value) {
  return timestamp(value.capturedAt, 'worker-local timestamp');
}

function stage(value, label, clockOrigin) {
  exactKeys(value, ['durationMs', 'finishedAt', 'startedAt'], label);
  const startedAt = timestamp(value.startedAt, `${label} start`);
  const finishedAt = timestamp(value.finishedAt, `${label} finish`);
  sameOrigin(startedAt, clockOrigin, `${label} start`);
  sameOrigin(finishedAt, clockOrigin, `${label} finish`);
  const durationMs = finishedAt.monotonicMs - startedAt.monotonicMs;
  if (durationMs < 0 || !approx(value.durationMs, durationMs)) {
    throw new Error(`${label} duration differs`);
  }
  return { startedAt, finishedAt, durationMs };
}

function timestamp(value, label) {
  exactKeys(value, ['epochMs', 'monotonicMs'], label);
  if (!nonnegative(value.monotonicMs) || !positive(value.epochMs)) {
    throw new Error(`${label} timestamp is invalid`);
  }
  return value;
}

function orderedTimestamps(values, label, clockOrigin) {
  const result = values.map((value, index) => timestamp(value, `${label} ${index}`));
  for (const [index, value] of result.entries()) {
    sameOrigin(value, clockOrigin, `${label} ${index}`);
    if (index > 0 && value.monotonicMs < result[index - 1].monotonicMs) {
      throw new Error(`${label} regresses at ${index}`);
    }
  }
  return result;
}

function gc(value, label) {
  exactKeys(value, ['byKind', 'count', 'durationMs', 'maxDurationMs'], `${label} GC`);
  if (
    !nonnegativeInteger(value.count) ||
    !nonnegative(value.durationMs) ||
    !nonnegative(value.maxDurationMs) ||
    !value.byKind ||
    typeof value.byKind !== 'object' ||
    Array.isArray(value.byKind)
  ) {
    throw new Error(`${label} GC is invalid`);
  }
}

function eventLoop(value, label) {
  exactKeys(value, ['active', 'idle', 'utilization'], `${label} event loop`);
  if (
    !nonnegative(value.active) ||
    !nonnegative(value.idle) ||
    !nonnegative(value.utilization) ||
    value.utilization > 1
  ) {
    throw new Error(`${label} event loop is invalid`);
  }
}

function positiveRss(value, label) {
  if (!value || typeof value !== 'object' || !positive(value.rss)) {
    throw new Error(`${label} RSS is invalid`);
  }
}

function positiveHeap(value, label) {
  if (!value || typeof value !== 'object' || !positive(value.heap_size_limit)) {
    throw new Error(`${label} heap is invalid`);
  }
}

function cpuUsage(value, label) {
  exactKeys(value, ['system', 'user'], label);
  if (!nonnegativeInteger(value.user) || !nonnegativeInteger(value.system)) {
    throw new Error(`${label} is invalid`);
  }
  return value;
}

function cpuDelta(start, end, key) {
  return {
    user: end[key].user - start[key].user,
    system: end[key].system - start[key].system,
  };
}

function countPluginKinds(kinds) {
  return {
    ordinaryJs: kinds.filter((kind) => kind === 'ordinary-js').length,
    parallelPlaceholders: kinds.filter((kind) => kind === 'parallel-placeholder').length,
    builtin: kinds.filter((kind) => kind === 'builtin').length,
  };
}

function countNativeKinds(kinds) {
  return {
    ordinaryJs: kinds.filter((kind) => kind === 'ordinary-js').length,
    parallelJs: kinds.filter((kind) => kind === 'parallel-js').length,
    builtin: kinds.filter((kind) => kind === 'builtin').length,
  };
}

function strings(value, label) {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.some((item) => typeof item !== 'string' || item.length === 0)
  ) {
    throw new Error(`${label} are missing`);
  }
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort((left, right) => left.localeCompare(right));
  const wanted = [...expected].sort((left, right) => left.localeCompare(right));
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${label} keys differ: ${JSON.stringify(actual)}`);
  }
}

function exactScope(value, expected, label) {
  exactKeys(value, Object.keys(expected), label);
  if (Object.entries(expected).some(([key, expectedValue]) => value[key] !== expectedValue)) {
    throw new Error(`${label} or RSS ownership claim is invalid`);
  }
}

const origin = (value) => value.epochMs - value.monotonicMs;
const approx = (left, right, tolerance = 1e-6) =>
  Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= tolerance;
const sameOrigin = (value, expected, label) => {
  if (!approx(origin(value), expected, 1e-3)) throw new Error(`${label} clock origin differs`);
};
const sameTimestamp = (left, right) =>
  left.monotonicMs === right.monotonicMs && left.epochMs === right.epochMs;
const sameCpu = (left, right) => left?.user === right?.user && left?.system === right?.system;
const nonnegative = (value) => Number.isFinite(value) && value >= 0;
const positive = (value) => Number.isFinite(value) && value > 0;
const nonnegativeInteger = (value) => Number.isSafeInteger(value) && value >= 0;
const positiveInteger = (value) => Number.isSafeInteger(value) && value > 0;
