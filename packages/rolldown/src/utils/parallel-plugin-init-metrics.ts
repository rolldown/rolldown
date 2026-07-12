import type {
  PerformanceObserver as NodePerformanceObserver,
  performance as NodePerformance,
} from 'node:perf_hooks';
import type { getHeapStatistics as getNodeHeapStatistics } from 'node:v8';

export const PARALLEL_PLUGIN_METRICS_VERSION = 1;

export type MetricsTimestamp = {
  monotonicMs: number;
  epochMs: number;
};

export type CpuUsageMicros = {
  user: number;
  system: number;
};

export type GcMetricsSnapshot = {
  count: number;
  durationMs: number;
  maxDurationMs: number;
  byKind: Record<
    string,
    { kind: number; count: number; durationMs: number; maxDurationMs: number }
  >;
};

export type MetricsRuntime = {
  performance: typeof NodePerformance;
  getHeapStatistics: typeof getNodeHeapStatistics;
  gcMetrics: ReturnType<typeof createGcMetricsCollector>;
};

export type ProcessMetricsSnapshot = ReturnType<typeof captureProcessMetrics>;

export type WorkerStageResourceSnapshot = ReturnType<typeof captureWorkerStageResourceSnapshot>;

export type MetricsStage = {
  startedAt: MetricsTimestamp;
  finishedAt: MetricsTimestamp;
  durationMs: number;
};

export type WorkerStageResourceWindow = {
  measurementClass: string;
  wallStage: MetricsStage;
  boundaryRefs: {
    before: string;
    after: string;
  };
  deltas: {
    processCpuUsageMicros: CpuUsageMicros;
    workerThreadCpuUsageMicros: CpuUsageMicros;
    processRssBytes: number;
    isolateUsedHeapSizeBytes: number;
    isolateGcCount: number;
    isolateGcDurationMs: number;
  };
  scope: {
    endpoints: string;
    processCpuUsage: string;
    workerThreadCpuUsage: string;
    processRss: string;
    isolateHeapAndGc: string;
  };
};

export type WorkerLauncherMetrics = {
  kind: 'rolldown_parallel_plugin_worker_launcher_metrics';
  version: 1;
  metricsId: number;
  scope: string;
  timeline: {
    launcherEntryAt: MetricsTimestamp;
    metricsRuntimeImportStartedAt: MetricsTimestamp;
    metricsRuntimeImportFinishedAt: MetricsTimestamp;
    runtimeAndBindingImportStartedAt: MetricsTimestamp;
    runtimeAndBindingImportFinishedAt: MetricsTimestamp;
  };
  stages: {
    metricsRuntimeImport: MetricsStage;
    runtimeAndBindingImport: MetricsStage;
  };
  resources: {
    afterMetricsRuntimeImportBeforeRuntimeAndBindingImport: ProcessMetricsSnapshot;
    afterRuntimeAndBindingImport: ProcessMetricsSnapshot;
  };
};

export type PluginBindingMetric = {
  pluginIndex: number;
  pluginName: string;
  pluginKind: 'ordinary-js' | 'parallel-placeholder' | 'builtin';
  stage: MetricsStage;
};

export type CreateBundlerOptionsMetrics = {
  kind: 'rolldown_create_bundler_options_metrics';
  version: 1;
  metricsId: number;
  measurementClass: string;
  pluginCounts: {
    inputBeforeOutputOptionsHook: number;
    outputBeforeOutputOptionsHook: number;
    ordinaryJs: number;
    parallelPlaceholders: number;
    builtin: number;
  };
  timeline: {
    createBundlerOptionsStartedAt: MetricsTimestamp;
    createBundlerOptionsFinishedAt: MetricsTimestamp;
  };
  stages: Record<string, MetricsStage>;
  pluginBinding: PluginBindingMetric[];
  resources: {
    scope: string;
    afterMetricsRuntimeSetupAtCreateBundlerOptionsStart: ProcessMetricsSnapshot;
    afterPluginNormalization: ProcessMetricsSnapshot;
    afterParallelPoolInitialization: ProcessMetricsSnapshot;
    afterInputBindingification: ProcessMetricsSnapshot;
    afterOutputBindingification: ProcessMetricsSnapshot;
    atCreateBundlerOptionsFinish: ProcessMetricsSnapshot;
  };
  isolationLimits: string[];
};

let nextMetricsId = 1;

export const parallelPluginMetricsEnabled = () =>
  process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS === 'json';

export const allocateParallelPluginMetricsId = () => nextMetricsId++;

export const metricsTimestamp = (): MetricsTimestamp => {
  const monotonicMs = performance.now();
  return { monotonicMs, epochMs: performance.timeOrigin + monotonicMs };
};

export const metricsStage = (
  startedAt: MetricsTimestamp,
  finishedAt: MetricsTimestamp,
): MetricsStage => ({
  startedAt,
  finishedAt,
  durationMs: finishedAt.monotonicMs - startedAt.monotonicMs,
});

export async function createMetricsRuntime(): Promise<MetricsRuntime> {
  const [{ performance, PerformanceObserver }, { getHeapStatistics }] = await Promise.all([
    import('node:perf_hooks'),
    import('node:v8'),
  ]);
  return {
    performance,
    getHeapStatistics,
    gcMetrics: createGcMetricsCollector(PerformanceObserver),
  };
}

export const captureProcessMetrics = (metricsRuntime: MetricsRuntime) => ({
  capturedAt: metricsTimestamp(),
  scope: {
    cpuUsage: 'whole process, including JS workers and native threads',
    mainThreadCpuUsage: 'current Node.js thread only',
    memoryUsage:
      'RSS is whole process; other process.memoryUsage fields follow current-thread/isolate semantics and are not worker ownership',
    heapStatistics: 'current V8 isolate only',
    eventLoopUtilization: 'current Node.js event loop only; this is not CPU time',
    gc: 'GC performance entries observed in this isolate after its metrics observer started',
  },
  processCpuUsageMicros: process.cpuUsage(),
  mainThreadCpuUsageMicros: process.threadCpuUsage(),
  processResourceUsage: process.resourceUsage(),
  processMemoryUsageBytes: process.memoryUsage(),
  isolateHeapStatistics: metricsRuntime.getHeapStatistics(),
  isolateEventLoopUtilization: metricsRuntime.performance.eventLoopUtilization(),
  isolateGc: metricsRuntime.gcMetrics.snapshot(),
});

export const captureWorkerStageResourceSnapshot = (metricsRuntime: MetricsRuntime) => {
  const captureStartedAt = metricsTimestamp();
  const snapshot = {
    processCpuUsageMicros: process.cpuUsage(),
    workerThreadCpuUsageMicros: process.threadCpuUsage(),
    processResourceUsage: process.resourceUsage(),
    processMemoryUsageBytes: process.memoryUsage(),
    isolateHeapStatistics: metricsRuntime.getHeapStatistics(),
    isolateEventLoopUtilization: metricsRuntime.performance.eventLoopUtilization(),
    isolateGc: metricsRuntime.gcMetrics.snapshot(),
  };
  const captureFinishedAt = metricsTimestamp();
  return {
    captureStartedAt,
    captureFinishedAt,
    scope: {
      processCpuUsage: 'whole process, including every JavaScript worker and native thread',
      workerThreadCpuUsage: 'current Node.js worker thread only',
      processMemoryUsage:
        'RSS is whole process and shared; other process.memoryUsage fields follow Node.js worker-thread semantics; none is worker, plugin, factory, or isolate ownership',
      isolateHeapStatistics: 'current worker V8 isolate only',
      isolateEventLoopUtilization: 'current worker event loop only; this is not CPU time',
      isolateGc:
        'GC performance entries observed in this worker after its metrics observer started',
    },
    ...snapshot,
  };
};

export const workerStageResourceWindow = (
  wallStage: MetricsStage,
  beforeBoundary: string,
  afterBoundary: string,
  before: WorkerStageResourceSnapshot,
  after: WorkerStageResourceSnapshot,
): WorkerStageResourceWindow => ({
  measurementClass:
    'synchronous bracketing resource snapshots; the resource delta contains the wall stage plus the two boundary-capture gaps and is not an exact wall-stage CPU or RSS attribution',
  wallStage,
  boundaryRefs: { before: beforeBoundary, after: afterBoundary },
  deltas: {
    processCpuUsageMicros: subtractCpuUsage(
      after.processCpuUsageMicros,
      before.processCpuUsageMicros,
    ),
    workerThreadCpuUsageMicros: subtractCpuUsage(
      after.workerThreadCpuUsageMicros,
      before.workerThreadCpuUsageMicros,
    ),
    processRssBytes: after.processMemoryUsageBytes.rss - before.processMemoryUsageBytes.rss,
    isolateUsedHeapSizeBytes:
      after.isolateHeapStatistics.used_heap_size - before.isolateHeapStatistics.used_heap_size,
    isolateGcCount: after.isolateGc.count - before.isolateGc.count,
    isolateGcDurationMs: after.isolateGc.durationMs - before.isolateGc.durationMs,
  },
  scope: {
    endpoints:
      'the before capture finishes before the wall stage starts and the after capture starts after the wall stage finishes',
    processCpuUsage:
      'whole-process cumulative-counter difference; concurrent workers, the Node.js main thread, native addons, and runtime threads are included and this is not plugin ownership',
    workerThreadCpuUsage:
      'current-worker cumulative-counter difference across the bracketing snapshots; boundary capture work and any interleaved work on this worker thread are included',
    processRss:
      'signed whole-process RSS difference; shared pages and concurrent allocation prevent worker, plugin, factory, or stage ownership',
    isolateHeapAndGc:
      'signed current-worker V8 used-heap difference and observed GC delta; native/shared memory is excluded, while interleaved work, GC timing, and worker state prevent plugin, factory, or stage ownership',
  },
});

export function createGcMetricsCollector(PerformanceObserver: typeof NodePerformanceObserver) {
  const totals = new Map<number, { count: number; durationMs: number; maxDurationMs: number }>();
  let count = 0;
  let durationMs = 0;
  let maxDurationMs = 0;

  const collect = (
    entries: Array<{ duration: number; detail?: { kind?: number }; kind?: number }>,
  ) => {
    for (const entry of entries) {
      const kind = entry.detail?.kind ?? entry.kind ?? 0;
      const kindTotals = totals.get(kind) ?? { count: 0, durationMs: 0, maxDurationMs: 0 };
      kindTotals.count += 1;
      kindTotals.durationMs += entry.duration;
      kindTotals.maxDurationMs = Math.max(kindTotals.maxDurationMs, entry.duration);
      totals.set(kind, kindTotals);
      count += 1;
      durationMs += entry.duration;
      maxDurationMs = Math.max(maxDurationMs, entry.duration);
    }
  };

  const observer = new PerformanceObserver((list) => collect(list.getEntries()));
  observer.observe({ entryTypes: ['gc'] });

  return {
    snapshot: (): GcMetricsSnapshot => {
      collect(observer.takeRecords());
      return {
        count,
        durationMs,
        maxDurationMs,
        byKind: Object.fromEntries(
          [...totals.entries()].map(([kind, value]) => [String(kind), { kind, ...value }]),
        ),
      };
    },
  };
}

export function validateWorkerLauncherMetrics(value: unknown): WorkerLauncherMetrics {
  const report = asRecord(value, 'worker launcher metrics');
  if (
    report.kind !== 'rolldown_parallel_plugin_worker_launcher_metrics' ||
    report.version !== PARALLEL_PLUGIN_METRICS_VERSION ||
    !validMetricsId(report.metricsId) ||
    typeof report.scope !== 'string' ||
    report.scope.length === 0
  ) {
    throw new Error('worker launcher metrics header is invalid');
  }
  const timeline = asRecord(report.timeline, 'worker launcher timeline');
  const orderedNames = [
    'launcherEntryAt',
    'metricsRuntimeImportStartedAt',
    'metricsRuntimeImportFinishedAt',
    'runtimeAndBindingImportStartedAt',
    'runtimeAndBindingImportFinishedAt',
  ];
  let previous = -Infinity;
  let launcherClockOrigin: number | undefined;
  for (const name of orderedNames) {
    const timestamp = validateTimestamp(timeline[name], `worker launcher ${name}`);
    if (timestamp.monotonicMs < previous) {
      throw new Error(`worker launcher timeline regresses at ${name}`);
    }
    const clockOrigin = timestamp.epochMs - timestamp.monotonicMs;
    if (launcherClockOrigin !== undefined && Math.abs(clockOrigin - launcherClockOrigin) > 1e-3) {
      throw new Error(`worker launcher clock origin changes at ${name}`);
    }
    launcherClockOrigin = clockOrigin;
    previous = timestamp.monotonicMs;
  }
  const stages = asRecord(report.stages, 'worker launcher stages');
  const metricsRuntimeImport = validateStage(
    stages.metricsRuntimeImport,
    'worker launcher metrics-runtime import',
  );
  const runtimeAndBindingImport = validateStage(
    stages.runtimeAndBindingImport,
    'worker launcher runtime-and-binding import',
  );
  for (const [label, stage] of [
    ['metrics-runtime import', metricsRuntimeImport],
    ['runtime-and-binding import', runtimeAndBindingImport],
  ] as const) {
    assertTimestampClockOrigin(
      stage.startedAt as MetricsTimestamp,
      launcherClockOrigin!,
      `worker launcher ${label} start`,
    );
    assertTimestampClockOrigin(
      stage.finishedAt as MetricsTimestamp,
      launcherClockOrigin!,
      `worker launcher ${label} finish`,
    );
  }
  if (
    (metricsRuntimeImport.startedAt as MetricsTimestamp).monotonicMs !==
      (timeline.metricsRuntimeImportStartedAt as MetricsTimestamp).monotonicMs ||
    (metricsRuntimeImport.finishedAt as MetricsTimestamp).monotonicMs !==
      (timeline.metricsRuntimeImportFinishedAt as MetricsTimestamp).monotonicMs ||
    (runtimeAndBindingImport.startedAt as MetricsTimestamp).monotonicMs !==
      (timeline.runtimeAndBindingImportStartedAt as MetricsTimestamp).monotonicMs ||
    (runtimeAndBindingImport.finishedAt as MetricsTimestamp).monotonicMs !==
      (timeline.runtimeAndBindingImportFinishedAt as MetricsTimestamp).monotonicMs
  ) {
    throw new Error('worker launcher stages do not match their timeline');
  }
  const resources = asRecord(report.resources, 'worker launcher resources');
  const beforeRuntime = validateProcessMetrics(
    resources.afterMetricsRuntimeImportBeforeRuntimeAndBindingImport,
    'worker launcher pre-runtime resources',
  );
  const afterRuntime = validateProcessMetrics(
    resources.afterRuntimeAndBindingImport,
    'worker launcher post-runtime resources',
  );
  if (
    beforeRuntime.capturedAt.monotonicMs <
      (timeline.metricsRuntimeImportFinishedAt as MetricsTimestamp).monotonicMs ||
    beforeRuntime.capturedAt.monotonicMs >
      (timeline.runtimeAndBindingImportStartedAt as MetricsTimestamp).monotonicMs ||
    afterRuntime.capturedAt.monotonicMs <
      (timeline.runtimeAndBindingImportFinishedAt as MetricsTimestamp).monotonicMs
  ) {
    throw new Error('worker launcher resources do not bracket runtime import');
  }
  if (
    Math.abs(
      beforeRuntime.capturedAt.epochMs -
        beforeRuntime.capturedAt.monotonicMs -
        launcherClockOrigin!,
    ) > 1e-3 ||
    Math.abs(
      afterRuntime.capturedAt.epochMs - afterRuntime.capturedAt.monotonicMs - launcherClockOrigin!,
    ) > 1e-3
  ) {
    throw new Error('worker launcher resource clocks do not correlate with its timeline');
  }
  return value as WorkerLauncherMetrics;
}

export function validateWorkerBootstrapMetrics(
  value: unknown,
  expectedThreadNumber?: number,
  expectedMetricsId?: number,
  expectedPluginIndexes?: number[],
) {
  const report = asRecord(value, 'worker bootstrap metrics');
  if (
    report.kind !== 'rolldown_parallel_plugin_worker_bootstrap_metrics' ||
    report.version !== PARALLEL_PLUGIN_METRICS_VERSION ||
    !validMetricsId(report.metricsId) ||
    (expectedMetricsId !== undefined && report.metricsId !== expectedMetricsId) ||
    !Number.isSafeInteger(report.threadNumber) ||
    (report.threadNumber as number) < 0 ||
    (expectedThreadNumber !== undefined && report.threadNumber !== expectedThreadNumber) ||
    !finiteNonnegative(report.measuredBootstrapMs) ||
    !finiteNonnegative(report.registerPluginsMs)
  ) {
    throw new Error('worker bootstrap metrics header is invalid');
  }
  const launcher = validateWorkerLauncherMetrics(report.launcher);
  if (launcher.metricsId !== report.metricsId) {
    throw new Error('worker launcher and bootstrap metrics identities do not match');
  }
  const timeline = asRecord(report.timeline, 'worker bootstrap timeline');
  const entryAt = validateTimestamp(timeline.entryAt, 'worker bootstrap entryAt');
  let previous = -Infinity;
  for (const name of [
    'launcherEntryAt',
    'runtimeAndBindingImportStartedAt',
    'runtimeAndBindingImportFinishedAt',
    'runtimeEntryAt',
    'bootstrapStartedAt',
    'registerStartedAt',
    'registerFinishedAt',
    'readyAt',
  ]) {
    const timestamp = validateTimestamp(timeline[name], `worker bootstrap ${name}`);
    if (timestamp.monotonicMs < previous) {
      throw new Error(`worker bootstrap timeline regresses at ${name}`);
    }
    previous = timestamp.monotonicMs;
  }
  const launcherEntryAt = validateTimestamp(
    timeline.launcherEntryAt,
    'worker bootstrap launcherEntryAt',
  );
  const registerStartedAt = validateTimestamp(
    timeline.registerStartedAt,
    'worker bootstrap register start',
  );
  const registerFinishedAt = validateTimestamp(
    timeline.registerFinishedAt,
    'worker bootstrap register finish',
  );
  const bootstrapStartedAt = validateTimestamp(
    timeline.bootstrapStartedAt,
    'worker bootstrap initialization start',
  );
  const readyAt = validateTimestamp(timeline.readyAt, 'worker bootstrap ready');
  if (
    entryAt.monotonicMs !== launcherEntryAt.monotonicMs ||
    Math.abs(
      (report.measuredBootstrapMs as number) -
        (registerFinishedAt.monotonicMs - launcherEntryAt.monotonicMs),
    ) > 1e-6 ||
    Math.abs(
      (report.registerPluginsMs as number) -
        (registerFinishedAt.monotonicMs - registerStartedAt.monotonicMs),
    ) > 1e-6
  ) {
    throw new Error('worker bootstrap aggregate durations do not match their timeline');
  }
  const registrationStage = validateStage(
    report.registrationStage,
    'worker plugin registration stage',
  );
  if (
    !sameTimestamp(registrationStage.startedAt as MetricsTimestamp, registerStartedAt) ||
    !sameTimestamp(registrationStage.finishedAt as MetricsTimestamp, registerFinishedAt)
  ) {
    throw new Error('worker plugin registration stage does not match its timeline');
  }
  const clock = asRecord(report.clockAlignment, 'worker bootstrap clock alignment');
  if (
    !finitePositive(clock.workerTimeOriginEpochMs) ||
    !finitePositive(clock.mainTimeOriginEpochMs) ||
    typeof clock.workerMinusMainTimeOriginMs !== 'number' ||
    !Number.isFinite(clock.workerMinusMainTimeOriginMs)
  ) {
    throw new Error('worker bootstrap clock alignment is invalid');
  }
  const workerOrigin = clock.workerTimeOriginEpochMs as number;
  const mainOrigin = clock.mainTimeOriginEpochMs as number;
  if (
    Math.abs((clock.workerMinusMainTimeOriginMs as number) - (workerOrigin - mainOrigin)) > 1e-6
  ) {
    throw new Error('worker bootstrap clock-origin difference is inconsistent');
  }
  for (const [name, rawTimestamp] of Object.entries(timeline)) {
    const timestamp = validateTimestamp(rawTimestamp, `worker bootstrap ${name}`);
    if (Math.abs(timestamp.epochMs - timestamp.monotonicMs - workerOrigin) > 1e-3) {
      throw new Error(`worker bootstrap ${name} is not correlated to the worker clock origin`);
    }
  }
  const launcherTimeline = asRecord(launcher.timeline, 'worker launcher/bootstrap shared timeline');
  for (const [launcherName, bootstrapName] of [
    ['launcherEntryAt', 'launcherEntryAt'],
    ['runtimeAndBindingImportStartedAt', 'runtimeAndBindingImportStartedAt'],
    ['runtimeAndBindingImportFinishedAt', 'runtimeAndBindingImportFinishedAt'],
  ] as const) {
    if (
      !sameTimestamp(
        validateTimestamp(launcherTimeline[launcherName], `worker launcher ${launcherName}`),
        validateTimestamp(timeline[bootstrapName], `worker bootstrap ${bootstrapName}`),
      )
    ) {
      throw new Error(`worker launcher/bootstrap timestamp mismatch at ${bootstrapName}`);
    }
  }
  if (!Array.isArray(report.plugins)) throw new Error('worker bootstrap plugins are missing');
  const indexes = new Set<number>();
  let earliestPluginImportStartedAt = Infinity;
  let latestPluginResourceBoundaryFinishedAt = -Infinity;
  for (const value of report.plugins) {
    const plugin = asRecord(value, 'worker bootstrap plugin');
    if (
      !Number.isSafeInteger(plugin.pluginIndex) ||
      (plugin.pluginIndex as number) < 0 ||
      indexes.has(plugin.pluginIndex as number) ||
      !finiteNonnegative(plugin.implementationImportMs) ||
      !finiteNonnegative(plugin.factoryMs) ||
      !finiteNonnegative(plugin.bindingifyMs)
    ) {
      throw new Error('worker bootstrap plugin metrics are invalid');
    }
    indexes.add(plugin.pluginIndex as number);
    const stages = asRecord(plugin.stages, 'worker bootstrap plugin stages');
    const implementationImport = validateStage(
      stages.implementationImport,
      'worker plugin implementation import',
    );
    const factory = validateStage(stages.factory, 'worker plugin factory');
    const bindingification = validateStage(
      stages.bindingifyPlugin,
      'worker plugin bindingification',
    );
    for (const [label, stage] of [
      ['implementation import', implementationImport],
      ['factory', factory],
      ['bindingification', bindingification],
    ] as const) {
      assertTimestampClockOrigin(
        stage.startedAt as MetricsTimestamp,
        workerOrigin,
        `worker plugin ${label} start`,
      );
      assertTimestampClockOrigin(
        stage.finishedAt as MetricsTimestamp,
        workerOrigin,
        `worker plugin ${label} finish`,
      );
    }
    const pluginTimeline = asRecord(plugin.timeline, 'worker bootstrap plugin timeline');
    const orderedPluginTimestamps = [
      pluginTimeline.importStartedAt,
      pluginTimeline.importFinishedAt,
      pluginTimeline.factoryStartedAt,
      pluginTimeline.factoryFinishedAt,
      pluginTimeline.bindingStartedAt,
      pluginTimeline.bindingFinishedAt,
    ].map((timestamp, index) =>
      validateTimestamp(timestamp, `worker plugin timeline position ${index}`),
    );
    if (
      orderedPluginTimestamps.some(
        (timestamp) => Math.abs(timestamp.epochMs - timestamp.monotonicMs - workerOrigin) > 1e-3,
      )
    ) {
      throw new Error('worker bootstrap plugin timeline does not correlate to the worker clock');
    }
    earliestPluginImportStartedAt = Math.min(
      earliestPluginImportStartedAt,
      orderedPluginTimestamps[0].monotonicMs,
    );
    if (
      Math.abs(
        (plugin.implementationImportMs as number) - (implementationImport.durationMs as number),
      ) > 1e-6 ||
      Math.abs((plugin.factoryMs as number) - (factory.durationMs as number)) > 1e-6 ||
      Math.abs((plugin.bindingifyMs as number) - (bindingification.durationMs as number)) > 1e-6 ||
      orderedPluginTimestamps.some(
        (timestamp, index) =>
          index > 0 && timestamp.monotonicMs < orderedPluginTimestamps[index - 1].monotonicMs,
      ) ||
      orderedPluginTimestamps[0].monotonicMs < bootstrapStartedAt.monotonicMs ||
      orderedPluginTimestamps.at(-1)!.monotonicMs > registerStartedAt.monotonicMs ||
      (implementationImport.startedAt as MetricsTimestamp).monotonicMs !==
        orderedPluginTimestamps[0].monotonicMs ||
      (implementationImport.finishedAt as MetricsTimestamp).monotonicMs !==
        orderedPluginTimestamps[1].monotonicMs ||
      (factory.startedAt as MetricsTimestamp).monotonicMs !==
        orderedPluginTimestamps[2].monotonicMs ||
      (factory.finishedAt as MetricsTimestamp).monotonicMs !==
        orderedPluginTimestamps[3].monotonicMs ||
      (bindingification.startedAt as MetricsTimestamp).monotonicMs !==
        orderedPluginTimestamps[4].monotonicMs ||
      (bindingification.finishedAt as MetricsTimestamp).monotonicMs !==
        orderedPluginTimestamps[5].monotonicMs
    ) {
      throw new Error('worker bootstrap plugin order, containment, or duration is invalid');
    }
    const resourceBoundaries = asRecord(
      plugin.resourceBoundaries,
      'worker bootstrap plugin resource boundaries',
    );
    const boundaryNames = [
      'beforeImplementationImport',
      'afterImplementationImportBeforeFactory',
      'afterFactoryBeforeBindingification',
      'afterBindingificationBeforeRegistration',
    ] as const;
    const validatedBoundaries = new Map<
      string,
      ReturnType<typeof validateWorkerStageResourceSnapshot>
    >();
    for (const name of boundaryNames) {
      validatedBoundaries.set(
        name,
        validateWorkerStageResourceSnapshot(
          resourceBoundaries[name],
          workerOrigin,
          `worker plugin ${name}`,
        ),
      );
    }
    const resourceWindows = asRecord(
      plugin.resourceWindows,
      'worker bootstrap plugin resource windows',
    );
    const windowDefinitions = [
      [
        'implementationImport',
        implementationImport,
        'beforeImplementationImport',
        'afterImplementationImportBeforeFactory',
      ],
      [
        'factory',
        factory,
        'afterImplementationImportBeforeFactory',
        'afterFactoryBeforeBindingification',
      ],
      [
        'bindingifyPlugin',
        bindingification,
        'afterFactoryBeforeBindingification',
        'afterBindingificationBeforeRegistration',
      ],
    ] as const;
    for (const [name, wallStage, beforeName, afterName] of windowDefinitions) {
      validateWorkerStageResourceWindow(
        resourceWindows[name],
        wallStage,
        workerOrigin,
        beforeName,
        afterName,
        validatedBoundaries.get(beforeName)!,
        validatedBoundaries.get(afterName)!,
        `worker plugin ${name} resources`,
      );
    }
    const firstBoundary = validatedBoundaries.get('beforeImplementationImport')!;
    const lastBoundary = validatedBoundaries.get('afterBindingificationBeforeRegistration')!;
    if (
      firstBoundary.captureStartedAt.monotonicMs < bootstrapStartedAt.monotonicMs ||
      lastBoundary.captureFinishedAt.monotonicMs > registerStartedAt.monotonicMs
    ) {
      throw new Error('worker plugin resource boundaries are outside bootstrap initialization');
    }
    latestPluginResourceBoundaryFinishedAt = Math.max(
      latestPluginResourceBoundaryFinishedAt,
      lastBoundary.captureFinishedAt.monotonicMs,
    );
  }
  if (
    expectedPluginIndexes !== undefined &&
    (indexes.size !== expectedPluginIndexes.length ||
      expectedPluginIndexes.some((index) => !indexes.has(index)))
  ) {
    throw new Error('worker bootstrap plugin indexes do not match the pool plugin indexes');
  }
  const workerLocalBefore = validateWorkerLocalMetrics(
    report.workerLocalBeforePluginInitialization,
    'worker local before plugin initialization',
  );
  const workerLocalAtReady = validateWorkerLocalMetrics(
    report.workerLocalAtReady,
    'worker local at ready',
  );
  assertTimestampClockOrigin(
    workerLocalBefore.capturedAt,
    workerOrigin,
    'worker local before plugin initialization',
  );
  assertTimestampClockOrigin(workerLocalAtReady.capturedAt, workerOrigin, 'worker local at ready');
  const registrationResources = asRecord(
    report.registrationResources,
    'worker registration resources',
  );
  const registrationBoundaries = asRecord(
    registrationResources.boundaries,
    'worker registration resource boundaries',
  );
  const beforeRegistration = validateWorkerStageResourceSnapshot(
    registrationBoundaries.beforeRegistration,
    workerOrigin,
    'worker before registration resources',
  );
  const afterRegistration = validateWorkerStageResourceSnapshot(
    registrationBoundaries.afterRegistration,
    workerOrigin,
    'worker after registration resources',
  );
  validateWorkerStageResourceWindow(
    registrationResources.window,
    registrationStage,
    workerOrigin,
    'beforeRegistration',
    'afterRegistration',
    beforeRegistration,
    afterRegistration,
    'worker registration resource window',
  );
  if (
    workerLocalBefore.capturedAt.monotonicMs < bootstrapStartedAt.monotonicMs ||
    workerLocalBefore.capturedAt.monotonicMs > earliestPluginImportStartedAt ||
    workerLocalAtReady.capturedAt.monotonicMs < registerFinishedAt.monotonicMs ||
    workerLocalAtReady.capturedAt.monotonicMs > readyAt.monotonicMs ||
    workerLocalAtReady.capturedAt.monotonicMs < workerLocalBefore.capturedAt.monotonicMs ||
    beforeRegistration.captureStartedAt.monotonicMs < latestPluginResourceBoundaryFinishedAt ||
    beforeRegistration.captureFinishedAt.monotonicMs > registerStartedAt.monotonicMs ||
    afterRegistration.captureStartedAt.monotonicMs < registerFinishedAt.monotonicMs ||
    afterRegistration.captureFinishedAt.monotonicMs > workerLocalAtReady.capturedAt.monotonicMs
  ) {
    throw new Error('worker-local snapshots are outside the worker bootstrap timeline');
  }
  if (
    !Array.isArray(report.isolationLimits) ||
    report.isolationLimits.length === 0 ||
    report.isolationLimits.some((value) => typeof value !== 'string' || value.length === 0)
  ) {
    throw new Error('worker bootstrap isolation limits are missing');
  }
  return report;
}

export function validateParallelPluginLifecycleMetrics(value: unknown) {
  const report = asRecord(value, 'parallel plugin lifecycle metrics');
  const initialization = report.kind === 'rolldown_parallel_plugin_init_metrics';
  const termination = report.kind === 'rolldown_parallel_plugin_termination_metrics';
  if (
    (!initialization && !termination) ||
    report.version !== PARALLEL_PLUGIN_METRICS_VERSION ||
    !validMetricsId(report.metricsId) ||
    !Number.isSafeInteger(report.workerCount) ||
    (report.workerCount as number) < 1 ||
    !finiteNonnegative(initialization ? report.poolInitializationMs : report.poolTerminationMs) ||
    !finitePositive(report.rssBeforeBytes) ||
    !finitePositive(report.rssAfterBytes) ||
    !Array.isArray(report.workers) ||
    report.workers.length !== report.workerCount
  ) {
    throw new Error('parallel plugin lifecycle metrics header is invalid');
  }
  if (!Number.isSafeInteger(report.pluginCount) || (report.pluginCount as number) < 1) {
    throw new Error('parallel plugin lifecycle plugin count is invalid');
  }
  if (
    !Array.isArray(report.parallelPluginIndexes) ||
    report.parallelPluginIndexes.length !== report.pluginCount
  ) {
    throw new Error('parallel plugin lifecycle plugin indexes are missing');
  }
  const pluginIndexes = new Set<number>();
  for (const index of report.parallelPluginIndexes) {
    if (
      !Number.isSafeInteger(index) ||
      (index as number) < 0 ||
      pluginIndexes.has(index as number)
    ) {
      throw new Error('parallel plugin lifecycle plugin index is invalid');
    }
    pluginIndexes.add(index as number);
  }
  const snapshots = asRecord(report.processSnapshots, 'parallel plugin lifecycle snapshots');
  if (typeof snapshots.scope !== 'string' || snapshots.scope.length === 0) {
    throw new Error('parallel plugin lifecycle snapshot scope is missing');
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
  let previousSnapshotFinishedAt = -Infinity;
  let mainClockOrigin: number | undefined;
  const validatedSnapshots = new Map<string, ReturnType<typeof validateLifecycleProcessSnapshot>>();
  for (const name of snapshotNames) {
    const snapshot = validateLifecycleProcessSnapshot(
      snapshots[name],
      `parallel plugin lifecycle ${name}`,
    );
    mainClockOrigin ??= timestampClockOrigin(snapshot.capturedAt);
    assertTimestampClockOrigin(
      snapshot.capturedAt,
      mainClockOrigin,
      `parallel plugin lifecycle ${name}`,
    );
    if (snapshot.captureStartedAt.monotonicMs < previousSnapshotFinishedAt) {
      throw new Error(`parallel plugin lifecycle snapshots regress at ${name}`);
    }
    previousSnapshotFinishedAt = snapshot.captureFinishedAt.monotonicMs;
    validatedSnapshots.set(name, snapshot);
  }
  const threadNumbers = new Set<number>();
  for (const value of report.workers) {
    const worker = asRecord(value, 'parallel plugin lifecycle worker');
    if (
      !Number.isSafeInteger(worker.threadNumber) ||
      (worker.threadNumber as number) < 0 ||
      (worker.threadNumber as number) >= (report.workerCount as number) ||
      threadNumbers.has(worker.threadNumber as number)
    ) {
      throw new Error('parallel plugin lifecycle worker identity is invalid');
    }
    threadNumbers.add(worker.threadNumber as number);
    const resourcesAtPoolReady = validateWorkerResourceCapture(
      worker.resourcesAtPoolReady,
      `worker ${worker.threadNumber as number} pool-ready resources`,
    );
    assertTimestampClockOrigin(
      resourcesAtPoolReady.startedAt,
      mainClockOrigin!,
      `worker ${worker.threadNumber as number} pool-ready resource start`,
    );
    assertTimestampClockOrigin(
      resourcesAtPoolReady.finishedAt,
      mainClockOrigin!,
      `worker ${worker.threadNumber as number} pool-ready resource finish`,
    );
    if (
      resourcesAtPoolReady.startedAt.monotonicMs <
        validatedSnapshots.get('allWorkersReady')!.capturedAt.monotonicMs ||
      resourcesAtPoolReady.finishedAt.monotonicMs >
        validatedSnapshots.get('resourceBaselineBeforeBuild')!.capturedAt.monotonicMs
    ) {
      throw new Error('parallel plugin pool-ready resource capture is outside main snapshots');
    }
    if (initialization) {
      if (!finiteNonnegative(worker.mainReadyMs)) {
        throw new Error('parallel plugin lifecycle worker ready duration is invalid');
      }
      const mainTimeline = asRecord(worker.mainTimeline, 'parallel plugin main worker timeline');
      const orderedMainTimeline = [
        'constructorStartedAt',
        'constructorReturnedAt',
        'onlineAt',
        'readyMessageAt',
      ].map((name) => validateTimestamp(mainTimeline[name], `parallel plugin main ${name}`));
      if (
        orderedMainTimeline.some(
          (timestamp, index) =>
            index > 0 && timestamp.monotonicMs < orderedMainTimeline[index - 1].monotonicMs,
        ) ||
        Math.abs(
          (worker.mainReadyMs as number) -
            (orderedMainTimeline[3].monotonicMs - orderedMainTimeline[0].monotonicMs),
        ) > 1e-6
      ) {
        throw new Error('parallel plugin main constructor, online, and ready order is invalid');
      }
      const mainOrigins = orderedMainTimeline.map(
        ({ epochMs, monotonicMs }) => epochMs - monotonicMs,
      );
      if (mainOrigins.some((origin) => Math.abs(origin - mainOrigins[0]) > 1e-3)) {
        throw new Error('parallel plugin main timeline clock correlation is invalid');
      }
      if (
        Math.abs(mainOrigins[0] - mainClockOrigin!) > 1e-3 ||
        orderedMainTimeline[0].monotonicMs <
          validatedSnapshots.get('beforeWorkerPool')!.capturedAt.monotonicMs ||
        orderedMainTimeline[3].monotonicMs >
          validatedSnapshots.get('allWorkersReady')!.capturedAt.monotonicMs
      ) {
        throw new Error('parallel plugin main worker timeline is outside lifecycle snapshots');
      }
      const bootstrap = validateWorkerBootstrapMetrics(
        worker.workerBootstrap,
        worker.threadNumber as number,
        report.metricsId as number,
        report.parallelPluginIndexes as number[],
      );
      const clock = asRecord(bootstrap.clockAlignment, 'parallel plugin worker clock');
      if (Math.abs((clock.mainTimeOriginEpochMs as number) - mainOrigins[0]) > 1e-3) {
        throw new Error('parallel plugin main and worker clock origins do not correlate');
      }
      const workerTimeline = asRecord(bootstrap.timeline, 'parallel plugin worker timeline');
      const launcherEntry = validateTimestamp(
        workerTimeline.launcherEntryAt,
        'parallel plugin worker launcher entry',
      );
      const workerReady = validateTimestamp(workerTimeline.readyAt, 'parallel plugin worker ready');
      if (
        launcherEntry.epochMs < orderedMainTimeline[0].epochMs ||
        workerReady.epochMs > orderedMainTimeline[3].epochMs
      ) {
        throw new Error('parallel plugin worker bootstrap is outside its main constructor window');
      }
    } else {
      const resourcesBeforeTermination = validateWorkerResourceCapture(
        worker.resourcesBeforeTermination,
        `worker ${worker.threadNumber as number} pre-termination resources`,
      );
      assertTimestampClockOrigin(
        resourcesBeforeTermination.startedAt,
        mainClockOrigin!,
        `worker ${worker.threadNumber as number} pre-termination resource start`,
      );
      assertTimestampClockOrigin(
        resourcesBeforeTermination.finishedAt,
        mainClockOrigin!,
        `worker ${worker.threadNumber as number} pre-termination resource finish`,
      );
      if (
        resourcesBeforeTermination.startedAt.monotonicMs <
          validatedSnapshots.get('beforeWorkerSnapshots')!.capturedAt.monotonicMs ||
        resourcesBeforeTermination.finishedAt.monotonicMs >
          validatedSnapshots.get('afterWorkerSnapshots')!.capturedAt.monotonicMs
      ) {
        throw new Error(
          'parallel plugin pre-termination resource capture is outside main snapshots',
        );
      }
      validateWorkerLocalMetrics(
        worker.workerLocalBeforeTermination,
        `worker ${worker.threadNumber as number} local pre-termination resources`,
      );
    }
  }
  validateCpuWindowDiagnostic(
    report.cpuWindows,
    initialization,
    report.workerCount as number,
    mainClockOrigin!,
    validatedSnapshots,
  );
  return report;
}

export function validateParallelPluginPostCloseMetrics(value: unknown) {
  const report = asRecord(value, 'parallel plugin post-close metrics');
  if (
    report.kind !== 'rolldown_parallel_plugin_post_close_metrics' ||
    report.version !== PARALLEL_PLUGIN_METRICS_VERSION ||
    !validMetricsId(report.metricsId) ||
    !Number.isSafeInteger(report.workerCount) ||
    (report.workerCount as number) < 1 ||
    !Number.isSafeInteger(report.pluginCount) ||
    (report.pluginCount as number) < 1 ||
    !Array.isArray(report.parallelPluginIndexes) ||
    report.parallelPluginIndexes.length !== report.pluginCount
  ) {
    throw new Error('parallel plugin post-close metrics header is invalid');
  }
  const pluginIndexes = new Set<number>();
  for (const index of report.parallelPluginIndexes) {
    if (
      !Number.isSafeInteger(index) ||
      (index as number) < 0 ||
      pluginIndexes.has(index as number)
    ) {
      throw new Error('parallel plugin post-close plugin index is invalid');
    }
    pluginIndexes.add(index as number);
  }
  const gc = asRecord(report.parentGc, 'parallel plugin post-close parent GC');
  if (
    gc.requestedPasses !== 2 ||
    typeof gc.available !== 'boolean' ||
    !Number.isSafeInteger(gc.executedPasses) ||
    (gc.available ? gc.executedPasses !== 2 : gc.executedPasses !== 0)
  ) {
    throw new Error('parallel plugin post-close parent GC record is invalid');
  }
  const snapshots = asRecord(
    report.processSnapshots,
    'parallel plugin post-close process snapshots',
  );
  if (
    typeof snapshots.scope !== 'string' ||
    !(snapshots.scope as string).includes('whole process') ||
    !(snapshots.scope as string).includes('not worker, plugin, factory, or isolate ownership')
  ) {
    throw new Error('parallel plugin post-close RSS ownership limitation is missing');
  }
  const names = ['afterTermination', 'afterBundlerCloseBeforeParentGc', 'parentPostGc'] as const;
  const validated = new Map<string, ReturnType<typeof validateLifecycleProcessSnapshot>>();
  let previousCaptureFinishedAt = -Infinity;
  let clockOrigin: number | undefined;
  for (const name of names) {
    const snapshot = validateLifecycleProcessSnapshot(
      snapshots[name],
      `parallel plugin post-close ${name}`,
    );
    clockOrigin ??= timestampClockOrigin(snapshot.capturedAt);
    assertTimestampClockOrigin(
      snapshot.capturedAt,
      clockOrigin,
      `parallel plugin post-close ${name}`,
    );
    if (snapshot.captureStartedAt.monotonicMs < previousCaptureFinishedAt) {
      throw new Error(`parallel plugin post-close snapshots regress at ${name}`);
    }
    previousCaptureFinishedAt = snapshot.captureFinishedAt.monotonicMs;
    validated.set(name, snapshot);
  }
  const cpuWindow = validateCpuProcessWindow(
    report.cpuWindow,
    'parallel plugin post-close CPU window',
  );
  if (
    !sameTimestamp(cpuWindow.startedAt, validated.get('afterTermination')!.capturedAt) ||
    !sameTimestamp(cpuWindow.finishedAt, validated.get('parentPostGc')!.capturedAt) ||
    !sameTimestamp(
      cpuWindow.startBounds.latestAt,
      validated.get('afterTermination')!.captureFinishedAt,
    ) ||
    !sameTimestamp(cpuWindow.endBounds.latestAt, validated.get('parentPostGc')!.captureFinishedAt)
  ) {
    throw new Error('parallel plugin post-close CPU endpoints do not match snapshots');
  }
  const expectedProcessCpu = subtractCpuUsage(
    validated.get('parentPostGc')!.processCpuUsageMicros as CpuUsageMicros,
    validated.get('afterTermination')!.processCpuUsageMicros as CpuUsageMicros,
  );
  const expectedMainThreadCpu = subtractCpuUsage(
    validated.get('parentPostGc')!.mainThreadCpuUsageMicros as CpuUsageMicros,
    validated.get('afterTermination')!.mainThreadCpuUsageMicros as CpuUsageMicros,
  );
  if (
    !sameCpu(cpuWindow.processCpuDeltaMicros, expectedProcessCpu) ||
    !sameCpu(cpuWindow.mainThreadCpuDeltaMicros, expectedMainThreadCpu)
  ) {
    throw new Error('parallel plugin post-close CPU deltas do not match snapshots');
  }
  const rss = asRecord(report.rss, 'parallel plugin post-close RSS');
  const afterTerminationRss = (
    validated.get('afterTermination')!.processMemoryUsageBytes as { rss: number }
  ).rss;
  const beforeGcRss = (
    validated.get('afterBundlerCloseBeforeParentGc')!.processMemoryUsageBytes as {
      rss: number;
    }
  ).rss;
  const postGcRss = (validated.get('parentPostGc')!.processMemoryUsageBytes as { rss: number }).rss;
  if (
    rss.afterTerminationBytes !== afterTerminationRss ||
    rss.afterBundlerCloseBeforeParentGcBytes !== beforeGcRss ||
    rss.parentPostGcRetainedBytes !== postGcRss ||
    rss.parentPostGcDeltaFromAfterTerminationBytes !== postGcRss - afterTerminationRss ||
    typeof rss.scope !== 'string' ||
    !(rss.scope as string).includes('never ownership')
  ) {
    throw new Error('parallel plugin post-close RSS values or scope are inconsistent');
  }
  if (
    !Array.isArray(report.isolationLimits) ||
    report.isolationLimits.length === 0 ||
    report.isolationLimits.some((item) => typeof item !== 'string' || item.length === 0) ||
    !report.isolationLimits.some((item) =>
      (item as string).includes('does not expose their exact read instants'),
    ) ||
    !report.isolationLimits.some((item) =>
      (item as string).includes('cannot assign retained memory'),
    ) ||
    !report.isolationLimits.some((item) => (item as string).includes('unavailable GC is recorded'))
  ) {
    throw new Error('parallel plugin post-close isolation limits are missing');
  }
  return report;
}

export function validateCreateBundlerOptionsMetrics(value: unknown): CreateBundlerOptionsMetrics {
  const report = asRecord(value, 'createBundlerOptions metrics');
  if (
    report.kind !== 'rolldown_create_bundler_options_metrics' ||
    report.version !== PARALLEL_PLUGIN_METRICS_VERSION ||
    !Number.isSafeInteger(report.metricsId) ||
    (report.metricsId as number) < 1 ||
    typeof report.measurementClass !== 'string' ||
    report.measurementClass.length === 0
  ) {
    throw new Error('createBundlerOptions metrics header is invalid');
  }
  const timeline = asRecord(report.timeline, 'createBundlerOptions timeline');
  const start = validateTimestamp(
    timeline.createBundlerOptionsStartedAt,
    'createBundlerOptions start',
  );
  const finish = validateTimestamp(
    timeline.createBundlerOptionsFinishedAt,
    'createBundlerOptions finish',
  );
  const mainClockOrigin = timestampClockOrigin(start);
  if (finish.monotonicMs < start.monotonicMs) {
    throw new Error('createBundlerOptions timeline regresses');
  }
  assertTimestampClockOrigin(finish, mainClockOrigin, 'createBundlerOptions finish');
  const counts = asRecord(report.pluginCounts, 'createBundlerOptions plugin counts');
  for (const name of [
    'inputBeforeOutputOptionsHook',
    'outputBeforeOutputOptionsHook',
    'ordinaryJs',
    'parallelPlaceholders',
    'builtin',
  ]) {
    if (!Number.isSafeInteger(counts[name]) || (counts[name] as number) < 0) {
      throw new Error(`createBundlerOptions plugin count ${name} is invalid`);
    }
  }
  const stages = asRecord(report.stages, 'createBundlerOptions stages');
  const orderedStageNames = [
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
  let previousStageFinish = start.monotonicMs;
  const validatedStages = new Map<string, Record<string, unknown>>();
  for (const name of orderedStageNames) {
    const stage = validateStage(stages[name], `createBundlerOptions ${name}`);
    const stageStart = (stage.startedAt as MetricsTimestamp).monotonicMs;
    const stageFinish = (stage.finishedAt as MetricsTimestamp).monotonicMs;
    assertTimestampClockOrigin(
      stage.startedAt as MetricsTimestamp,
      mainClockOrigin,
      `createBundlerOptions ${name} start`,
    );
    assertTimestampClockOrigin(
      stage.finishedAt as MetricsTimestamp,
      mainClockOrigin,
      `createBundlerOptions ${name} finish`,
    );
    if (stageStart < previousStageFinish || stageFinish > finish.monotonicMs) {
      throw new Error(`createBundlerOptions stage ordering is invalid at ${name}`);
    }
    previousStageFinish = stageFinish;
    validatedStages.set(name, stage);
  }
  if (!Array.isArray(report.pluginBinding)) {
    throw new Error('createBundlerOptions plugin binding metrics are missing');
  }
  const indexes = new Set<number>();
  for (const entry of report.pluginBinding) {
    const metric = asRecord(entry, 'plugin binding metric');
    if (
      !Number.isSafeInteger(metric.pluginIndex) ||
      (metric.pluginIndex as number) < 0 ||
      indexes.has(metric.pluginIndex as number) ||
      typeof metric.pluginName !== 'string' ||
      metric.pluginName.length === 0 ||
      !['ordinary-js', 'parallel-placeholder', 'builtin'].includes(metric.pluginKind as string)
    ) {
      throw new Error('plugin binding metric identity is invalid');
    }
    indexes.add(metric.pluginIndex as number);
    const pluginStage = validateStage(
      metric.stage,
      `plugin binding ${metric.pluginIndex as number}`,
    );
    assertTimestampClockOrigin(
      pluginStage.startedAt as MetricsTimestamp,
      mainClockOrigin,
      `plugin binding ${metric.pluginIndex as number} start`,
    );
    assertTimestampClockOrigin(
      pluginStage.finishedAt as MetricsTimestamp,
      mainClockOrigin,
      `plugin binding ${metric.pluginIndex as number} finish`,
    );
    const inputBindingStage = validatedStages.get('bindingifyInputOptions')!;
    if (
      (pluginStage.startedAt as MetricsTimestamp).monotonicMs <
        (inputBindingStage.startedAt as MetricsTimestamp).monotonicMs ||
      (pluginStage.finishedAt as MetricsTimestamp).monotonicMs >
        (inputBindingStage.finishedAt as MetricsTimestamp).monotonicMs
    ) {
      throw new Error('plugin binding metric falls outside bindingifyInputOptions');
    }
  }
  const finalCounts = {
    ordinaryJs: report.pluginBinding.filter(({ pluginKind }) => pluginKind === 'ordinary-js')
      .length,
    parallelPlaceholders: report.pluginBinding.filter(
      ({ pluginKind }) => pluginKind === 'parallel-placeholder',
    ).length,
    builtin: report.pluginBinding.filter(({ pluginKind }) => pluginKind === 'builtin').length,
  };
  if (
    counts.ordinaryJs !== finalCounts.ordinaryJs ||
    counts.parallelPlaceholders !== finalCounts.parallelPlaceholders ||
    counts.builtin !== finalCounts.builtin
  ) {
    throw new Error('createBundlerOptions plugin counts do not match plugin binding entries');
  }
  const resources = asRecord(report.resources, 'createBundlerOptions resources');
  if (typeof resources.scope !== 'string' || resources.scope.length === 0) {
    throw new Error('createBundlerOptions resource scope is missing');
  }
  const resourceNames = [
    'afterMetricsRuntimeSetupAtCreateBundlerOptionsStart',
    'afterPluginNormalization',
    'afterParallelPoolInitialization',
    'afterInputBindingification',
    'afterOutputBindingification',
    'atCreateBundlerOptionsFinish',
  ] as const;
  let previousResourceTime = start.monotonicMs;
  const validatedResources = new Map<string, ProcessMetricsSnapshot>();
  for (const name of resourceNames) {
    const snapshot = validateProcessMetrics(resources[name], `createBundlerOptions ${name}`);
    assertTimestampClockOrigin(
      snapshot.capturedAt,
      mainClockOrigin,
      `createBundlerOptions ${name}`,
    );
    if (
      snapshot.capturedAt.monotonicMs < previousResourceTime ||
      snapshot.capturedAt.monotonicMs > finish.monotonicMs
    ) {
      throw new Error(`createBundlerOptions resource containment/order is invalid at ${name}`);
    }
    previousResourceTime = snapshot.capturedAt.monotonicMs;
    validatedResources.set(name, snapshot);
  }
  const resourceStageRelations = [
    [
      'afterMetricsRuntimeSetupAtCreateBundlerOptionsStart',
      'metricsRuntimeSetup',
      'normalizeInputPluginOption',
    ],
    ['afterPluginNormalization', 'normalizePluginObjects', 'parallelPoolInitialization'],
    ['afterParallelPoolInitialization', 'parallelPoolInitialization', 'pluginContextConstruction'],
    ['afterInputBindingification', 'bindingifyInputOptions', 'bindingifyOutputOptions'],
    ['afterOutputBindingification', 'bindingifyOutputOptions', undefined],
    ['atCreateBundlerOptionsFinish', 'bindingifyOutputOptions', undefined],
  ] as const;
  for (const [resourceName, precedingStageName, followingStageName] of resourceStageRelations) {
    const capturedAt = validatedResources.get(resourceName)!.capturedAt.monotonicMs;
    if (
      capturedAt <
        (validatedStages.get(precedingStageName)!.finishedAt as MetricsTimestamp).monotonicMs ||
      (followingStageName !== undefined &&
        capturedAt >
          (validatedStages.get(followingStageName)!.startedAt as MetricsTimestamp).monotonicMs)
    ) {
      throw new Error(
        `createBundlerOptions resource ${resourceName} is outside its adjacent stages`,
      );
    }
  }
  if (
    !Array.isArray(report.isolationLimits) ||
    report.isolationLimits.length === 0 ||
    report.isolationLimits.some((item) => typeof item !== 'string' || item.length === 0)
  ) {
    throw new Error('createBundlerOptions isolation limits are missing');
  }
  return value as CreateBundlerOptionsMetrics;
}

export function validateNativePluginRegistrationMetrics(value: unknown) {
  const report = asRecord(value, 'native plugin registration metrics');
  if (
    report.kind !== 'rolldown_native_plugin_registration_metrics' ||
    report.version !== PARALLEL_PLUGIN_METRICS_VERSION ||
    !validMetricsId(report.metricsId) ||
    report.boundary !==
      'after BindingBundlerOptions destructuring, before registry transfer, through BundlerConfig construction, synchronously before ClassicBundler::create_bundle and Bundle::scan' ||
    !finiteNonnegative(report.nativeNormalizationTotalMs) ||
    !finiteNonnegative(report.nativePluginMaterializationMs) ||
    !Array.isArray(report.plugins) ||
    !Number.isSafeInteger(report.ordinaryJsPluginCount) ||
    !Number.isSafeInteger(report.parallelJsPluginCount) ||
    !Number.isSafeInteger(report.builtinPluginCount) ||
    typeof report.parallelRegistryPresent !== 'boolean' ||
    !Number.isSafeInteger(report.workerManagerWorkerCount) ||
    (report.workerManagerWorkerCount as number) < 0 ||
    typeof report.scope !== 'string' ||
    report.scope.length === 0
  ) {
    throw new Error('native plugin registration metrics header is invalid');
  }
  const stages = asRecord(report.stages, 'native plugin registration stages');
  for (const name of [
    'registryTransferMs',
    'workerManagerConstructionMs',
    'bindingOptionNormalizationMs',
    'pluginMaterializationMs',
  ]) {
    if (!finiteNonnegative(stages[name])) {
      throw new Error(`native plugin registration stage ${name} is invalid`);
    }
  }
  if (
    stages.pluginMaterializationMs !== report.nativePluginMaterializationMs ||
    (stages.pluginMaterializationMs as number) > (stages.bindingOptionNormalizationMs as number) ||
    (stages.registryTransferMs as number) +
      (stages.workerManagerConstructionMs as number) +
      (stages.bindingOptionNormalizationMs as number) >
      (report.nativeNormalizationTotalMs as number) + 1e-3
  ) {
    throw new Error('native plugin registration stage containment is invalid');
  }
  const relationships = asRecord(
    report.stageRelationships,
    'native plugin registration stage relationships',
  );
  if (Object.keys(relationships).length !== 4) {
    throw new Error('native plugin registration stage relationships are incomplete');
  }
  const indexes = new Set<number>();
  for (const value of report.plugins) {
    const plugin = asRecord(value, 'native plugin registration entry');
    if (
      !Number.isSafeInteger(plugin.index) ||
      (plugin.index as number) < 0 ||
      indexes.has(plugin.index as number) ||
      typeof plugin.name !== 'string' ||
      plugin.name.length === 0 ||
      !['ordinary-js', 'parallel-js', 'builtin'].includes(plugin.kind as string) ||
      !finiteNonnegative(plugin.materializationMs)
    ) {
      throw new Error('native plugin registration entry is invalid');
    }
    indexes.add(plugin.index as number);
  }
  const counts = {
    ordinary: report.plugins.filter(({ kind }) => kind === 'ordinary-js').length,
    parallel: report.plugins.filter(({ kind }) => kind === 'parallel-js').length,
    builtin: report.plugins.filter(({ kind }) => kind === 'builtin').length,
  };
  if (
    report.ordinaryJsPluginCount !== counts.ordinary ||
    report.parallelJsPluginCount !== counts.parallel ||
    report.builtinPluginCount !== counts.builtin
  ) {
    throw new Error('native plugin registration counts do not match plugin entries');
  }
  if (
    (report.parallelJsPluginCount as number) > 0 !== (report.parallelRegistryPresent as boolean) ||
    (report.parallelJsPluginCount as number) > 0 !== (report.workerManagerWorkerCount as number) > 0
  ) {
    throw new Error('native parallel registry and WorkerManager counts are inconsistent');
  }
  return report;
}

export function writeValidatedMetrics(
  prefix: string,
  report: unknown,
  validate: (value: unknown) => void,
) {
  validate(report);
  process.stderr.write(`[${prefix}] ${JSON.stringify(report)}\n`);
}

function validateStage(value: unknown, label: string) {
  const stage = asRecord(value, label);
  const startedAt = validateTimestamp(stage.startedAt, `${label} start`);
  const finishedAt = validateTimestamp(stage.finishedAt, `${label} finish`);
  if (
    finishedAt.monotonicMs < startedAt.monotonicMs ||
    !finiteNonnegative(stage.durationMs) ||
    Math.abs((stage.durationMs as number) - (finishedAt.monotonicMs - startedAt.monotonicMs)) > 1e-6
  ) {
    throw new Error(`${label} duration is invalid`);
  }
  return stage;
}

function validateWorkerStageResourceWindow(
  value: unknown,
  expectedWallStage: Record<string, unknown>,
  expectedClockOrigin: number,
  expectedBeforeBoundary: string,
  expectedAfterBoundary: string,
  before: ReturnType<typeof validateWorkerStageResourceSnapshot>,
  after: ReturnType<typeof validateWorkerStageResourceSnapshot>,
  label: string,
) {
  const window = asRecord(value, label);
  if (
    window.measurementClass !==
    'synchronous bracketing resource snapshots; the resource delta contains the wall stage plus the two boundary-capture gaps and is not an exact wall-stage CPU or RSS attribution'
  ) {
    throw new Error(`${label} measurement class is invalid`);
  }
  const wallStage = validateStage(window.wallStage, `${label} wall stage`);
  if (
    !sameTimestamp(
      wallStage.startedAt as MetricsTimestamp,
      expectedWallStage.startedAt as MetricsTimestamp,
    ) ||
    !sameTimestamp(
      wallStage.finishedAt as MetricsTimestamp,
      expectedWallStage.finishedAt as MetricsTimestamp,
    )
  ) {
    throw new Error(`${label} does not match its wall stage`);
  }
  const boundaryRefs = asRecord(window.boundaryRefs, `${label} boundary references`);
  if (
    boundaryRefs.before !== expectedBeforeBoundary ||
    boundaryRefs.after !== expectedAfterBoundary
  ) {
    throw new Error(`${label} boundary references are invalid`);
  }
  assertTimestampClockOrigin(
    before.captureStartedAt,
    expectedClockOrigin,
    `${label} before boundary`,
  );
  assertTimestampClockOrigin(
    after.captureFinishedAt,
    expectedClockOrigin,
    `${label} after boundary`,
  );
  if (
    before.captureFinishedAt.monotonicMs > (wallStage.startedAt as MetricsTimestamp).monotonicMs ||
    after.captureStartedAt.monotonicMs < (wallStage.finishedAt as MetricsTimestamp).monotonicMs ||
    after.captureStartedAt.monotonicMs < before.captureFinishedAt.monotonicMs
  ) {
    throw new Error(`${label} resource snapshots do not bracket the wall stage`);
  }
  const expectedDeltas = {
    processCpuUsageMicros: subtractCpuUsage(
      after.processCpuUsageMicros,
      before.processCpuUsageMicros,
    ),
    workerThreadCpuUsageMicros: subtractCpuUsage(
      after.workerThreadCpuUsageMicros,
      before.workerThreadCpuUsageMicros,
    ),
    processRssBytes: after.processMemoryUsageBytes.rss - before.processMemoryUsageBytes.rss,
    isolateUsedHeapSizeBytes:
      after.isolateHeapStatistics.used_heap_size - before.isolateHeapStatistics.used_heap_size,
    isolateGcCount: after.isolateGc.count - before.isolateGc.count,
    isolateGcDurationMs: after.isolateGc.durationMs - before.isolateGc.durationMs,
  };
  const deltas = asRecord(window.deltas, `${label} deltas`);
  validateCpu(deltas.processCpuUsageMicros, `${label} process CPU delta`);
  validateCpu(deltas.workerThreadCpuUsageMicros, `${label} worker-thread CPU delta`);
  if (
    !finiteNumber(deltas.processRssBytes) ||
    !finiteNumber(deltas.isolateUsedHeapSizeBytes) ||
    !Number.isSafeInteger(deltas.isolateGcCount) ||
    (deltas.isolateGcCount as number) < 0 ||
    !finiteNonnegative(deltas.isolateGcDurationMs) ||
    !sameCpu(
      deltas.processCpuUsageMicros as CpuUsageMicros,
      expectedDeltas.processCpuUsageMicros,
    ) ||
    !sameCpu(
      deltas.workerThreadCpuUsageMicros as CpuUsageMicros,
      expectedDeltas.workerThreadCpuUsageMicros,
    ) ||
    deltas.processRssBytes !== expectedDeltas.processRssBytes ||
    deltas.isolateUsedHeapSizeBytes !== expectedDeltas.isolateUsedHeapSizeBytes ||
    deltas.isolateGcCount !== expectedDeltas.isolateGcCount ||
    Math.abs((deltas.isolateGcDurationMs as number) - expectedDeltas.isolateGcDurationMs) > 1e-6
  ) {
    throw new Error(`${label} resource deltas are inconsistent`);
  }
  const scope = asRecord(window.scope, `${label} scope`);
  for (const name of [
    'endpoints',
    'processCpuUsage',
    'workerThreadCpuUsage',
    'processRss',
    'isolateHeapAndGc',
  ]) {
    if (typeof scope[name] !== 'string' || (scope[name] as string).length === 0) {
      throw new Error(`${label} scope is incomplete`);
    }
  }
  if (
    !(scope.processCpuUsage as string).includes('not plugin ownership') ||
    !(scope.processRss as string).includes('prevent worker, plugin, factory, or stage ownership') ||
    !(scope.isolateHeapAndGc as string).includes('prevent plugin, factory, or stage ownership')
  ) {
    throw new Error(`${label} ownership limitation is missing`);
  }
  return window;
}

function validateWorkerStageResourceSnapshot(
  value: unknown,
  expectedClockOrigin: number,
  label: string,
) {
  const snapshot = asRecord(value, label);
  const captureStartedAt = validateTimestamp(snapshot.captureStartedAt, `${label} capture start`);
  const captureFinishedAt = validateTimestamp(
    snapshot.captureFinishedAt,
    `${label} capture finish`,
  );
  assertTimestampClockOrigin(captureStartedAt, expectedClockOrigin, `${label} capture start`);
  assertTimestampClockOrigin(captureFinishedAt, expectedClockOrigin, `${label} capture finish`);
  if (captureFinishedAt.monotonicMs < captureStartedAt.monotonicMs) {
    throw new Error(`${label} capture timeline regresses`);
  }
  validateCpu(snapshot.processCpuUsageMicros, `${label} process CPU`);
  validateCpu(snapshot.workerThreadCpuUsageMicros, `${label} worker-thread CPU`);
  asRecord(snapshot.processResourceUsage, `${label} process resource usage`);
  const processMemoryUsageBytes = asRecord(
    snapshot.processMemoryUsageBytes,
    `${label} process memory`,
  );
  if (!finitePositive(processMemoryUsageBytes.rss)) {
    throw new Error(`${label} process RSS is invalid`);
  }
  const isolateHeapStatistics = asRecord(snapshot.isolateHeapStatistics, `${label} isolate heap`);
  if (
    !finitePositive(isolateHeapStatistics.heap_size_limit) ||
    !finiteNonnegative(isolateHeapStatistics.used_heap_size)
  ) {
    throw new Error(`${label} isolate heap is invalid`);
  }
  asRecord(snapshot.isolateEventLoopUtilization, `${label} event-loop utilization`);
  const isolateGc = asRecord(snapshot.isolateGc, `${label} isolate GC`);
  if (
    !Number.isSafeInteger(isolateGc.count) ||
    (isolateGc.count as number) < 0 ||
    !finiteNonnegative(isolateGc.durationMs) ||
    !finiteNonnegative(isolateGc.maxDurationMs)
  ) {
    throw new Error(`${label} isolate GC is invalid`);
  }
  const scope = asRecord(snapshot.scope, `${label} scope`);
  for (const name of [
    'processCpuUsage',
    'workerThreadCpuUsage',
    'processMemoryUsage',
    'isolateHeapStatistics',
    'isolateEventLoopUtilization',
    'isolateGc',
  ]) {
    if (typeof scope[name] !== 'string' || (scope[name] as string).length === 0) {
      throw new Error(`${label} scope is incomplete`);
    }
  }
  if (
    !(scope.processMemoryUsage as string).includes(
      'none is worker, plugin, factory, or isolate ownership',
    )
  ) {
    throw new Error(`${label} RSS ownership limitation is missing`);
  }
  return {
    captureStartedAt,
    captureFinishedAt,
    processCpuUsageMicros: snapshot.processCpuUsageMicros as CpuUsageMicros,
    workerThreadCpuUsageMicros: snapshot.workerThreadCpuUsageMicros as CpuUsageMicros,
    processMemoryUsageBytes: processMemoryUsageBytes as unknown as { rss: number },
    isolateHeapStatistics: isolateHeapStatistics as unknown as { used_heap_size: number },
    isolateGc: isolateGc as unknown as { count: number; durationMs: number },
  };
}

function validateLifecycleProcessSnapshot(value: unknown, label: string) {
  const snapshot = asRecord(value, label);
  const capturedAt = validateTimestamp(snapshot.capturedAt, `${label} timestamp`);
  const captureStartedAt = validateTimestamp(snapshot.captureStartedAt, `${label} capture start`);
  const captureFinishedAt = validateTimestamp(
    snapshot.captureFinishedAt,
    `${label} capture finish`,
  );
  if (
    !sameTimestamp(capturedAt, captureStartedAt) ||
    captureFinishedAt.monotonicMs < captureStartedAt.monotonicMs
  ) {
    throw new Error(`${label} capture bounds are invalid`);
  }
  assertTimestampClockOrigin(
    captureFinishedAt,
    timestampClockOrigin(captureStartedAt),
    `${label} capture finish`,
  );
  const scope = asRecord(snapshot.scope, `${label} scope`);
  if (
    typeof scope.endpoints !== 'string' ||
    !(scope.endpoints as string).includes('does not expose each exact counter-read instant')
  ) {
    throw new Error(`${label} endpoint limitation is missing`);
  }
  validateCpu(snapshot.processCpuUsageMicros, `${label} process CPU`);
  validateCpu(snapshot.mainThreadCpuUsageMicros, `${label} main-thread CPU`);
  const memory = asRecord(snapshot.processMemoryUsageBytes, `${label} memory`);
  if (!finitePositive(memory.rss)) throw new Error(`${label} RSS is invalid`);
  const heap = asRecord(snapshot.mainIsolateHeapStatistics, `${label} main heap`);
  if (!finitePositive(heap.heap_size_limit)) throw new Error(`${label} main heap is invalid`);
  const gc = asRecord(snapshot.mainIsolateGc, `${label} main GC`);
  if (!Number.isSafeInteger(gc.count) || (gc.count as number) < 0) {
    throw new Error(`${label} main GC is invalid`);
  }
  return {
    ...snapshot,
    capturedAt,
    captureStartedAt,
    captureFinishedAt,
    processCpuUsageMicros: snapshot.processCpuUsageMicros as CpuUsageMicros,
    mainThreadCpuUsageMicros: snapshot.mainThreadCpuUsageMicros as CpuUsageMicros,
    processMemoryUsageBytes: memory as unknown as { rss: number },
  };
}

function validateTimestamp(value: unknown, label: string): MetricsTimestamp {
  const timestamp = asRecord(value, label);
  if (!finiteNonnegative(timestamp.monotonicMs) || !finiteNonnegative(timestamp.epochMs)) {
    throw new Error(`${label} is invalid`);
  }
  return timestamp as MetricsTimestamp;
}

function validateProcessMetrics(value: unknown, label: string): ProcessMetricsSnapshot {
  const snapshot = asRecord(value, label);
  const capturedAt = validateTimestamp(snapshot.capturedAt, `${label} timestamp`);
  validateCpu(snapshot.processCpuUsageMicros, `${label} process CPU`);
  validateCpu(snapshot.mainThreadCpuUsageMicros, `${label} thread CPU`);
  const memory = asRecord(snapshot.processMemoryUsageBytes, `${label} memory`);
  if (!finitePositive(memory.rss)) throw new Error(`${label} RSS is invalid`);
  const heap = asRecord(snapshot.isolateHeapStatistics, `${label} heap`);
  if (!finitePositive(heap.heap_size_limit)) throw new Error(`${label} heap is invalid`);
  const gc = asRecord(snapshot.isolateGc, `${label} GC`);
  if (
    !Number.isSafeInteger(gc.count) ||
    (gc.count as number) < 0 ||
    !finiteNonnegative(gc.durationMs) ||
    !finiteNonnegative(gc.maxDurationMs)
  ) {
    throw new Error(`${label} GC is invalid`);
  }
  return { ...(snapshot as ProcessMetricsSnapshot), capturedAt };
}

function validateWorkerResourceCapture(value: unknown, label: string) {
  const capture = asRecord(value, label);
  if (capture.ok !== true) throw new Error(`${label} failed`);
  const snapshot = asRecord(capture.snapshot, `${label} snapshot`);
  const startedAt = validateTimestamp(snapshot.captureStartedAt, `${label} start`);
  const finishedAt = validateTimestamp(snapshot.captureFinishedAt, `${label} finish`);
  if (finishedAt.monotonicMs < startedAt.monotonicMs) {
    throw new Error(`${label} timeline regresses`);
  }
  validateCpu(snapshot.cpuUsageMicros, `${label} CPU`);
  const heap = asRecord(snapshot.heapStatistics, `${label} heap`);
  if (!finitePositive(heap.heap_size_limit)) throw new Error(`${label} heap is invalid`);
  asRecord(snapshot.eventLoopUtilization, `${label} event-loop utilization`);
  return { startedAt, finishedAt };
}

function validateWorkerLocalMetrics(value: unknown, label: string) {
  const snapshot = asRecord(value, label);
  const capturedAt = validateTimestamp(snapshot.capturedAt, `${label} timestamp`);
  validateCpu(snapshot.threadCpuUsageMicros, `${label} thread CPU`);
  const memory = asRecord(snapshot.processMemoryUsageBytes, `${label} memory`);
  if (!finitePositive(memory.rss)) throw new Error(`${label} RSS is invalid`);
  const heap = asRecord(snapshot.heapStatistics, `${label} heap`);
  if (!finitePositive(heap.heap_size_limit)) throw new Error(`${label} heap is invalid`);
  asRecord(snapshot.eventLoopUtilization, `${label} event-loop utilization`);
  const gc = asRecord(snapshot.gc, `${label} GC`);
  if (!Number.isSafeInteger(gc.count) || (gc.count as number) < 0) {
    throw new Error(`${label} GC is invalid`);
  }
  return { capturedAt };
}

function validateCpuWindowDiagnostic(
  value: unknown,
  initialization: boolean,
  workerCount: number,
  expectedClockOrigin: number,
  lifecycleSnapshots: Map<string, ReturnType<typeof validateLifecycleProcessSnapshot>>,
) {
  const diagnostic = asRecord(value, 'parallel plugin CPU window diagnostic');
  if (
    diagnostic.measurementClass !==
      'asynchronous-bracketing-diagnostic; not exact CPU attribution' ||
    diagnostic.completeWorkerCoverage !== true ||
    typeof diagnostic.scope !== 'string' ||
    !(diagnostic.scope as string).includes('process-snapshot capture bounds') ||
    !(diagnostic.scope as string).includes('never subtracted')
  ) {
    throw new Error('parallel plugin CPU window diagnostic header is invalid');
  }
  const outer = validateCpuProcessWindow(
    diagnostic.outerProcessWindow,
    'parallel plugin outer CPU process window',
  );
  const inner = initialization
    ? undefined
    : validateCpuProcessWindow(
        diagnostic.innerProcessWindow,
        'parallel plugin inner CPU process window',
      );
  for (const [label, timestamp] of [
    ['outer start', outer.startedAt],
    ['outer finish', outer.finishedAt],
    ...(inner
      ? ([
          ['inner start', inner.startedAt],
          ['inner finish', inner.finishedAt],
        ] as const)
      : []),
  ] as const) {
    assertTimestampClockOrigin(timestamp, expectedClockOrigin, `parallel plugin CPU ${label}`);
  }
  if (
    inner &&
    (inner.startedAt.monotonicMs < outer.startedAt.monotonicMs ||
      inner.finishedAt.monotonicMs > outer.finishedAt.monotonicMs)
  ) {
    throw new Error('parallel plugin inner CPU process window is outside the outer window');
  }
  const expectedOuterStart = lifecycleSnapshots.get(
    initialization ? 'beforeWorkerPool' : 'allWorkersReady',
  )!.capturedAt;
  const expectedOuterFinish = lifecycleSnapshots.get(
    initialization ? 'resourceBaselineBeforeBuild' : 'afterWorkerSnapshots',
  )!.capturedAt;
  if (
    !sameTimestamp(outer.startedAt, expectedOuterStart) ||
    !sameTimestamp(outer.finishedAt, expectedOuterFinish) ||
    (inner &&
      (!sameTimestamp(
        inner.startedAt,
        lifecycleSnapshots.get('resourceBaselineBeforeBuild')!.capturedAt,
      ) ||
        !sameTimestamp(
          inner.finishedAt,
          lifecycleSnapshots.get('beforeWorkerSnapshots')!.capturedAt,
        )))
  ) {
    throw new Error('parallel plugin CPU windows do not match lifecycle process snapshots');
  }
  const expectedOuterStartSnapshot = lifecycleSnapshots.get(
    initialization ? 'beforeWorkerPool' : 'allWorkersReady',
  )!;
  const expectedOuterFinishSnapshot = lifecycleSnapshots.get(
    initialization ? 'resourceBaselineBeforeBuild' : 'afterWorkerSnapshots',
  )!;
  if (
    !sameTimestamp(outer.startBounds.latestAt, expectedOuterStartSnapshot.captureFinishedAt) ||
    !sameTimestamp(outer.endBounds.latestAt, expectedOuterFinishSnapshot.captureFinishedAt) ||
    (inner &&
      (!sameTimestamp(
        inner.startBounds.latestAt,
        lifecycleSnapshots.get('resourceBaselineBeforeBuild')!.captureFinishedAt,
      ) ||
        !sameTimestamp(
          inner.endBounds.latestAt,
          lifecycleSnapshots.get('beforeWorkerSnapshots')!.captureFinishedAt,
        )))
  ) {
    throw new Error('parallel plugin CPU capture bounds changed their process snapshots');
  }
  if (!Array.isArray(diagnostic.workerSamples) || diagnostic.workerSamples.length !== workerCount) {
    throw new Error('parallel plugin CPU worker samples are missing');
  }
  const threadNumbers = new Set<number>();
  const summed = { user: 0, system: 0 };
  for (const rawSample of diagnostic.workerSamples) {
    const sample = asRecord(rawSample, 'parallel plugin CPU worker sample');
    if (
      sample.ok !== true ||
      !Number.isSafeInteger(sample.threadNumber) ||
      (sample.threadNumber as number) < 0 ||
      (sample.threadNumber as number) >= workerCount ||
      threadNumbers.has(sample.threadNumber as number) ||
      typeof sample.measurementClass !== 'string' ||
      sample.measurementClass.length === 0 ||
      typeof sample.relationToProcessWindows !== 'string' ||
      sample.relationToProcessWindows.length === 0
    ) {
      throw new Error('parallel plugin CPU worker sample header is invalid');
    }
    threadNumbers.add(sample.threadNumber as number);
    const startBounds = validateTimestampBounds(
      sample.startBounds,
      'parallel plugin CPU worker start bounds',
    );
    const endBounds = validateTimestampBounds(
      sample.endBounds,
      'parallel plugin CPU worker end bounds',
    );
    for (const [label, timestamp] of [
      ['start earliest', startBounds.earliestAt],
      ['start latest', startBounds.latestAt],
      ['end earliest', endBounds.earliestAt],
      ['end latest', endBounds.latestAt],
    ] as const) {
      assertTimestampClockOrigin(
        timestamp,
        expectedClockOrigin,
        `parallel plugin CPU worker ${label}`,
      );
    }
    if (
      startBounds.earliestAt.monotonicMs < outer.startedAt.monotonicMs ||
      endBounds.latestAt.monotonicMs > outer.finishedAt.monotonicMs ||
      endBounds.earliestAt.monotonicMs < startBounds.latestAt.monotonicMs ||
      (inner &&
        (startBounds.latestAt.monotonicMs > inner.startedAt.monotonicMs ||
          endBounds.earliestAt.monotonicMs < inner.finishedAt.monotonicMs))
    ) {
      throw new Error('parallel plugin CPU worker sample is outside its declared brackets');
    }
    const cpu = asRecord(sample.cpuDeltaMicros, 'parallel plugin CPU worker delta');
    validateCpu(cpu, 'parallel plugin CPU worker delta');
    summed.user += cpu.user as number;
    summed.system += cpu.system as number;
  }
  const reportedSum = asRecord(
    diagnostic.summedObservedWorkerThreadCpuMicros,
    'parallel plugin CPU worker sum',
  );
  validateCpu(reportedSum, 'parallel plugin CPU worker sum');
  if (reportedSum.user !== summed.user || reportedSum.system !== summed.system) {
    throw new Error('parallel plugin CPU worker sum is inconsistent');
  }
}

function validateCpuProcessWindow(value: unknown, label: string) {
  const window = asRecord(value, label);
  if (
    window.measurementClass !==
      'synchronous snapshot-bracketed cumulative-counter difference; exact CPU counter read instants are not exposed' ||
    typeof window.scope !== 'string' ||
    !(window.scope as string).includes('neither delta is plugin or native ownership')
  ) {
    throw new Error(`${label} endpoint or ownership scope is invalid`);
  }
  const startedAt = validateTimestamp(window.startedAt, `${label} start`);
  const finishedAt = validateTimestamp(window.finishedAt, `${label} finish`);
  if (finishedAt.monotonicMs < startedAt.monotonicMs) {
    throw new Error(`${label} regresses`);
  }
  validateCpu(window.processCpuDeltaMicros, `${label} process CPU`);
  validateCpu(window.mainThreadCpuDeltaMicros, `${label} main-thread CPU`);
  const captureBounds = asRecord(window.captureBounds, `${label} capture bounds`);
  const startBounds = validateTimestampBounds(captureBounds.start, `${label} start capture bounds`);
  const endBounds = validateTimestampBounds(captureBounds.end, `${label} end capture bounds`);
  assertTimestampClockOrigin(
    startBounds.latestAt,
    timestampClockOrigin(startBounds.earliestAt),
    `${label} start capture finish`,
  );
  assertTimestampClockOrigin(
    endBounds.latestAt,
    timestampClockOrigin(endBounds.earliestAt),
    `${label} end capture finish`,
  );
  if (
    !sameTimestamp(startedAt, startBounds.earliestAt) ||
    !sameTimestamp(finishedAt, endBounds.earliestAt) ||
    endBounds.earliestAt.monotonicMs < startBounds.latestAt.monotonicMs
  ) {
    throw new Error(`${label} capture bounds do not match its labeled endpoints`);
  }
  return {
    startedAt,
    finishedAt,
    startBounds,
    endBounds,
    processCpuDeltaMicros: window.processCpuDeltaMicros as CpuUsageMicros,
    mainThreadCpuDeltaMicros: window.mainThreadCpuDeltaMicros as CpuUsageMicros,
  };
}

function validateTimestampBounds(value: unknown, label: string) {
  const bounds = asRecord(value, label);
  const earliestAt = validateTimestamp(bounds.earliestAt, `${label} earliest`);
  const latestAt = validateTimestamp(bounds.latestAt, `${label} latest`);
  if (
    latestAt.monotonicMs < earliestAt.monotonicMs ||
    typeof bounds.meaning !== 'string' ||
    bounds.meaning.length === 0
  ) {
    throw new Error(`${label} is invalid`);
  }
  return { earliestAt, latestAt };
}

function validateCpu(value: unknown, label: string) {
  const cpu = asRecord(value, label);
  if (!finiteNonnegative(cpu.user) || !finiteNonnegative(cpu.system)) {
    throw new Error(`${label} is invalid`);
  }
}

function subtractCpuUsage(end: CpuUsageMicros, start: CpuUsageMicros): CpuUsageMicros {
  return { user: end.user - start.user, system: end.system - start.system };
}

function sameCpu(left: CpuUsageMicros, right: CpuUsageMicros) {
  return left.user === right.user && left.system === right.system;
}

function asRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function finiteNonnegative(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

function finitePositive(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0;
}

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function validMetricsId(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 1;
}

function timestampClockOrigin(timestamp: MetricsTimestamp) {
  return timestamp.epochMs - timestamp.monotonicMs;
}

function assertTimestampClockOrigin(
  timestamp: MetricsTimestamp,
  expectedOrigin: number,
  label: string,
) {
  if (Math.abs(timestampClockOrigin(timestamp) - expectedOrigin) > 1e-3) {
    throw new Error(`${label} clock origin is inconsistent`);
  }
}

function sameTimestamp(left: MetricsTimestamp, right: MetricsTimestamp) {
  return left.monotonicMs === right.monotonicMs && left.epochMs === right.epochMs;
}
