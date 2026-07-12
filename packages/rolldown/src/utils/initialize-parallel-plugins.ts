import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';
import {
  createMetricsRuntime,
  metricsTimestamp,
  parallelPluginMetricsEnabled,
  validateParallelPluginLifecycleMetrics,
  validateParallelPluginPostCloseMetrics,
  validateWorkerBootstrapMetrics,
  type MetricsRuntime,
  type MetricsTimestamp,
} from './parallel-plugin-init-metrics';

export type { MetricsTimestamp } from './parallel-plugin-init-metrics';

export type WorkerData = {
  registryId: number;
  pluginInfos: ParallelPluginInfo[];
  threadNumber: number;
  metricsEnabled?: true;
  metricsId?: number;
  metricsMainTimeOriginEpochMs?: number;
};

export type WorkerMetricsSnapshotRequest = {
  type: 'metrics-snapshot-request';
  requestId: number;
};

export type WorkerMetricsSnapshotResponse = {
  type: 'metrics-snapshot-response';
  requestId: number;
  metrics: unknown;
};

type ParallelPluginInfo = {
  index: number;
  fileUrl: string;
  options: unknown;
};

type InitializedWorker = {
  worker: Worker;
  threadNumber: number;
  mainTimeline: {
    constructorStartedAt: MetricsTimestamp;
    constructorReturnedAt: MetricsTimestamp;
    onlineAt: MetricsTimestamp;
    readyMessageAt: MetricsTimestamp;
  };
  workerBootstrap: unknown;
  resourcesAtPoolReady?: WorkerResourceCapture;
};

type CpuUsageMicros = {
  user: number;
  system: number;
};

type EventLoopUtilization = {
  idle: number;
  active: number;
  utilization: number;
};

type WorkerResourceSnapshot = {
  captureStartedAt: MetricsTimestamp;
  captureFinishedAt: MetricsTimestamp;
  cpuUsageMicros: CpuUsageMicros;
  heapStatistics: Awaited<ReturnType<Worker['getHeapStatistics']>>;
  eventLoopUtilization: EventLoopUtilization;
};

type WorkerResourceCapture =
  | { ok: true; snapshot: WorkerResourceSnapshot }
  | { ok: false; error: string };

type ProcessSnapshot = ReturnType<typeof captureProcessSnapshot>;

let nextMetricsRequestId = 1;

export async function initializeParallelPlugins(
  plugins: RolldownPlugin[],
  inheritedMetricsRuntime?: MetricsRuntime,
  metricsId?: number,
): Promise<
  | {
      registry: ParallelJsPluginRegistry;
      stopWorkers: () => Promise<void>;
      finalizeMetricsAfterClose?: () => Promise<void>;
    }
  | undefined
> {
  const pluginInfos: ParallelPluginInfo[] = [];
  for (const [index, plugin] of plugins.entries()) {
    if ('_parallel' in plugin) {
      const { fileUrl, options } = plugin._parallel;
      pluginInfos.push({ index, fileUrl, options });
    }
  }
  if (pluginInfos.length <= 0) {
    return undefined;
  }

  const metricsEnabled = parallelPluginMetricsEnabled();
  if (metricsEnabled && metricsId === undefined) {
    throw new Error('parallel plugin initialization metrics identity is missing');
  }
  const metricsRuntime = metricsEnabled
    ? (inheritedMetricsRuntime ?? (await createMetricsRuntime()))
    : undefined;
  const initializationStartedAt = metricsEnabled ? performance.now() : 0;
  const processBeforeWorkerPool = metricsRuntime
    ? captureProcessSnapshot(metricsRuntime)
    : undefined;
  const count = availableParallelism();
  const parallelJsPluginRegistry = new ParallelJsPluginRegistry(count);
  const registryId = parallelJsPluginRegistry.id;

  if (!metricsEnabled) {
    const workers = await initializeWorkers(registryId, count, pluginInfos);
    return {
      registry: parallelJsPluginRegistry,
      stopWorkers: async () => {
        await Promise.all(workers.map((worker) => worker.terminate()));
      },
    };
  }

  const initializedWorkers = await initializeWorkersWithMetrics(
    registryId,
    count,
    pluginInfos,
    metricsId!,
  );
  const workers = initializedWorkers.map(({ worker }) => worker);
  let processWhenAllWorkersReady: ProcessSnapshot;
  let processAtPoolReady: ProcessSnapshot;
  try {
    if (process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT === 'pool-after-worker-creation') {
      throw new Error('injected metrics fault after parallel plugin worker creation');
    }
    const poolInitializationMs = performance.now() - initializationStartedAt;
    processWhenAllWorkersReady = captureProcessSnapshot(metricsRuntime!);
    const resourcesAtPoolReady = await Promise.all(
      initializedWorkers.map(({ worker }) => captureWorkerResources(worker)),
    );
    for (const [index, resources] of resourcesAtPoolReady.entries()) {
      initializedWorkers[index].resourcesAtPoolReady = resources;
    }
    processAtPoolReady = captureProcessSnapshot(metricsRuntime!);
    await writeMetrics('rolldown_parallel_plugin_init_metrics', {
      metricsId,
      workerCount: count,
      pluginCount: pluginInfos.length,
      parallelPluginIndexes: pluginInfos.map(({ index }) => index),
      poolInitializationMs,
      rssBeforeBytes: processBeforeWorkerPool!.processMemoryUsageBytes.rss,
      rssAfterBytes: processAtPoolReady.processMemoryUsageBytes.rss,
      rssScope: 'whole process; the before/after delta is not worker ownership',
      processSnapshots: {
        scope: 'whole process; RSS is not attributed to an isolate or worker',
        beforeWorkerPool: processBeforeWorkerPool,
        allWorkersReady: processWhenAllWorkersReady,
        resourceBaselineBeforeBuild: processAtPoolReady,
      },
      cpuWindows: createCpuWindowDiagnostic({
        phase: 'initialization',
        outerStart: processBeforeWorkerPool!,
        outerEnd: processAtPoolReady,
        workers: initializedWorkers,
      }),
      workers: initializedWorkers.map(
        ({ threadNumber, mainTimeline, workerBootstrap, resourcesAtPoolReady }) => ({
          threadNumber,
          mainReadyMs:
            mainTimeline.readyMessageAt.monotonicMs - mainTimeline.constructorStartedAt.monotonicMs,
          mainTimeline,
          workerBootstrap,
          resourcesAtPoolReady,
        }),
      ),
    });
  } catch (error) {
    await Promise.all(workers.map((worker) => worker.terminate()));
    throw error;
  }
  let processAfterTerminationForPostClose: ProcessSnapshot | undefined;
  let stopWorkersPromise: Promise<void> | undefined;
  const stopWorkersOnce = async () => {
    const terminationStartedAt = performance.now();
    const processBeforeWorkerSnapshots = captureProcessSnapshot(metricsRuntime!);
    const beforeTermination = await Promise.all(
      initializedWorkers.map(({ worker }) => captureWorkerBeforeTermination(worker)),
    );
    const processAfterWorkerSnapshots = captureProcessSnapshot(metricsRuntime!);
    await Promise.all(workers.map((worker) => worker.terminate()));
    const processAfterTermination = captureProcessSnapshot(metricsRuntime!);
    processAfterTerminationForPostClose = processAfterTermination;
    await writeMetrics('rolldown_parallel_plugin_termination_metrics', {
      metricsId,
      workerCount: count,
      pluginCount: pluginInfos.length,
      parallelPluginIndexes: pluginInfos.map(({ index }) => index),
      poolTerminationMs: performance.now() - terminationStartedAt,
      rssBeforeBytes: processBeforeWorkerSnapshots.processMemoryUsageBytes.rss,
      rssAfterBytes: processAfterTermination.processMemoryUsageBytes.rss,
      rssScope: 'whole process; the before/after delta is not worker ownership',
      processSnapshots: {
        scope: 'whole process; RSS is not attributed to an isolate or worker',
        allWorkersReady: processWhenAllWorkersReady,
        resourceBaselineBeforeBuild: processAtPoolReady,
        beforeWorkerSnapshots: processBeforeWorkerSnapshots,
        afterWorkerSnapshots: processAfterWorkerSnapshots,
        afterTermination: processAfterTermination,
      },
      cpuWindows: createCpuWindowDiagnostic({
        phase: 'lifetime-through-pre-termination-snapshot',
        outerStart: processWhenAllWorkersReady,
        outerEnd: processAfterWorkerSnapshots,
        innerStart: processAtPoolReady,
        innerEnd: processBeforeWorkerSnapshots,
        workers: initializedWorkers,
        workerEnds: beforeTermination.map(({ resources }) => resources),
      }),
      workers: initializedWorkers.map(({ threadNumber, resourcesAtPoolReady }, index) => ({
        threadNumber,
        resourcesAtPoolReady,
        resourcesBeforeTermination: beforeTermination[index].resources,
        workerLocalBeforeTermination: beforeTermination[index].workerLocal,
      })),
    });
  };
  const stopWorkers = () => (stopWorkersPromise ??= stopWorkersOnce());

  let finalizeMetricsAfterClosePromise: Promise<void> | undefined;
  const finalizeMetricsAfterCloseOnce = async () => {
    if (!stopWorkersPromise) {
      throw new Error('parallel plugin post-close metrics require completed worker termination');
    }
    await stopWorkersPromise;
    if (!processAfterTerminationForPostClose) {
      throw new Error(
        'parallel plugin post-close metrics require a post-termination process snapshot',
      );
    }
    const afterBundlerCloseBeforeParentGc = captureProcessSnapshot(metricsRuntime!);
    const parentGc = globalThis.gc;
    const parentGcAvailable = typeof parentGc === 'function';
    let executedPasses = 0;
    if (parentGcAvailable) {
      parentGc();
      executedPasses++;
      parentGc();
      executedPasses++;
    }
    const parentPostGc = captureProcessSnapshot(metricsRuntime!);
    await writePostCloseMetrics({
      kind: 'rolldown_parallel_plugin_post_close_metrics',
      version: 1,
      metricsId,
      workerCount: count,
      pluginCount: pluginInfos.length,
      parallelPluginIndexes: pluginInfos.map(({ index }) => index),
      parentGc: {
        requestedPasses: 2,
        available: parentGcAvailable,
        executedPasses,
      },
      processSnapshots: {
        scope:
          'whole process across worker termination, native bundler close, and parent GC requests; RSS is not worker, plugin, factory, or isolate ownership',
        afterTermination: processAfterTerminationForPostClose,
        afterBundlerCloseBeforeParentGc,
        parentPostGc,
      },
      cpuWindow: cpuProcessWindow(processAfterTerminationForPostClose, parentPostGc),
      rss: {
        afterTerminationBytes: processAfterTerminationForPostClose.processMemoryUsageBytes.rss,
        afterBundlerCloseBeforeParentGcBytes:
          afterBundlerCloseBeforeParentGc.processMemoryUsageBytes.rss,
        parentPostGcRetainedBytes: parentPostGc.processMemoryUsageBytes.rss,
        parentPostGcDeltaFromAfterTerminationBytes:
          parentPostGc.processMemoryUsageBytes.rss -
          processAfterTerminationForPostClose.processMemoryUsageBytes.rss,
        scope:
          'signed whole-process observations across termination, native close, and parent GC; shared and allocator-retained pages mean the delta is never ownership',
      },
      isolationLimits: [
        'parentPostGc is available only when Node.js starts with --expose-gc; unavailable GC is recorded instead of silently claiming a post-GC boundary',
        'whole-process RSS includes the main isolate, native allocator retention, runtime threads, loaded code, and shared pages; it cannot assign retained memory to a worker, plugin, factory, or initialization stage',
        'process and main-thread CPU counters are read synchronously within each reported capture bound, but Node.js does not expose their exact read instants; their delta includes termination-report serialization and flush, native bundler close, two explicit GC requests, metrics capture, and any concurrent runtime work',
      ],
    });
  };
  const finalizeMetricsAfterClose = () =>
    (finalizeMetricsAfterClosePromise ??= finalizeMetricsAfterCloseOnce());

  return { registry: parallelJsPluginRegistry, stopWorkers, finalizeMetricsAfterClose };
}

async function initializeWorkers(
  registryId: number,
  count: number,
  pluginInfos: ParallelPluginInfo[],
): Promise<Worker[]> {
  const results = await Promise.allSettled(
    Array.from({ length: count }, (_, i) => initializeWorker(registryId, pluginInfos, i)),
  );
  const workers = results.flatMap((result) =>
    result.status === 'fulfilled' ? [result.value] : [],
  );
  const failure = results.find((result) => result.status === 'rejected');
  if (failure) {
    await Promise.all(workers.map((worker) => worker.terminate()));
    throw failure.reason;
  }
  return workers;
}

async function initializeWorkersWithMetrics(
  registryId: number,
  count: number,
  pluginInfos: ParallelPluginInfo[],
  metricsId: number,
): Promise<InitializedWorker[]> {
  const results = await Promise.allSettled(
    Array.from({ length: count }, (_, i) =>
      initializeWorkerWithMetrics(registryId, pluginInfos, i, metricsId),
    ),
  );
  const workers = results.flatMap((result) =>
    result.status === 'fulfilled' ? [result.value] : [],
  );
  const failure = results.find((result) => result.status === 'rejected');
  if (failure) {
    await Promise.all(workers.map(({ worker }) => worker.terminate()));
    throw failure.reason;
  }
  return workers;
}

async function initializeWorker(
  registryId: number,
  pluginInfos: ParallelPluginInfo[],
  threadNumber: number,
) {
  const urlString = import.meta.resolve('#parallel-plugin-worker');
  const workerData: WorkerData = {
    registryId,
    pluginInfos,
    threadNumber,
  };

  let worker: Worker | undefined;
  try {
    worker = new Worker(new URL(urlString), { workerData });
    await waitForWorkerReady(worker);
    return worker;
  } catch (e) {
    await worker?.terminate();
    throw e;
  }
}

async function initializeWorkerWithMetrics(
  registryId: number,
  pluginInfos: ParallelPluginInfo[],
  threadNumber: number,
  metricsId: number,
) {
  const urlString = import.meta.resolve('#parallel-plugin-worker-metrics');
  const workerData: WorkerData = {
    registryId,
    pluginInfos,
    threadNumber,
    metricsEnabled: true,
    metricsId,
    metricsMainTimeOriginEpochMs: performance.timeOrigin,
  };

  let worker: Worker | undefined;
  try {
    const constructorStartedAt = metricsTimestamp();
    worker = new Worker(new URL(urlString), { workerData });
    const constructorReturnedAt = metricsTimestamp();
    const { onlineAt, message } = await waitForWorkerReady(worker);
    const readyMessageAt = metricsTimestamp();
    validateWorkerBootstrapMetrics(
      message.metrics,
      threadNumber,
      metricsId,
      pluginInfos.map(({ index }) => index),
    );
    return {
      worker,
      threadNumber,
      mainTimeline: {
        constructorStartedAt,
        constructorReturnedAt,
        onlineAt,
        readyMessageAt,
      },
      workerBootstrap: message.metrics,
    };
  } catch (e) {
    await worker?.terminate();
    throw e;
  }
}

const waitForWorkerReady = (worker: Worker) =>
  new Promise<{
    onlineAt: MetricsTimestamp;
    message: { type: string; error?: unknown; metrics?: unknown };
  }>((resolve, reject) => {
    let onlineAt: MetricsTimestamp | undefined;
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error('timed out waiting for parallel plugin worker readiness'));
    }, 30_000);
    const onOnline = () => {
      onlineAt = metricsTimestamp();
    };
    const onMessage = (message: { type: string; error?: unknown; metrics?: unknown }) => {
      cleanup();
      if (message.type === 'error') {
        reject(message.error);
      } else if (!onlineAt) {
        reject(new Error('parallel plugin worker became ready before its online event'));
      } else {
        resolve({ onlineAt, message });
      }
    };
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };
    const onExit = (code: number) => {
      cleanup();
      reject(new Error(`parallel plugin worker exited with code ${code} before readiness`));
    };
    const cleanup = () => {
      clearTimeout(timeout);
      worker.off('online', onOnline);
      worker.off('message', onMessage);
      worker.off('error', onError);
      worker.off('exit', onExit);
    };
    worker.once('online', onOnline);
    worker.once('message', onMessage);
    worker.once('error', onError);
    worker.once('exit', onExit);
  });

const captureWorkerResources = async (worker: Worker): Promise<WorkerResourceCapture> => {
  try {
    if (worker.threadId === -1) throw new Error('worker has already exited');
    const captureStartedAt = metricsTimestamp();
    const eventLoopUtilization = worker.performance.eventLoopUtilization();
    const [cpuUsageMicros, heapStatistics] = await withMetricsTimeout(
      Promise.all([worker.cpuUsage(), worker.getHeapStatistics()]),
      'worker resource snapshot',
    );
    return {
      ok: true,
      snapshot: {
        captureStartedAt,
        captureFinishedAt: metricsTimestamp(),
        cpuUsageMicros,
        heapStatistics,
        eventLoopUtilization,
      },
    };
  } catch (error) {
    return { ok: false, error: formatMetricsError(error) };
  }
};

const captureWorkerBeforeTermination = async (worker: Worker) => {
  const [resources, workerLocal] = await Promise.all([
    captureWorkerResources(worker),
    requestWorkerLocalMetrics(worker),
  ]);
  return { resources, workerLocal };
};

const requestWorkerLocalMetrics = async (worker: Worker): Promise<unknown> => {
  const requestId = nextMetricsRequestId++;
  try {
    if (worker.threadId === -1) throw new Error('worker has already exited');
    return await new Promise<unknown>((resolve, reject) => {
      const timeout = setTimeout(() => {
        cleanup();
        reject(new Error('timed out waiting for worker metrics snapshot'));
      }, 10_000);

      const onMessage = (message: WorkerMetricsSnapshotResponse) => {
        if (message.type !== 'metrics-snapshot-response' || message.requestId !== requestId) {
          return;
        }
        cleanup();
        resolve(message.metrics);
      };
      const onError = (error: Error) => {
        cleanup();
        reject(error);
      };
      const onExit = (code: number) => {
        cleanup();
        reject(new Error(`worker exited with code ${code} before metrics snapshot`));
      };
      const cleanup = () => {
        clearTimeout(timeout);
        worker.off('message', onMessage);
        worker.off('error', onError);
        worker.off('exit', onExit);
      };

      worker.on('message', onMessage);
      worker.once('error', onError);
      worker.once('exit', onExit);
      worker.postMessage({
        type: 'metrics-snapshot-request',
        requestId,
      } satisfies WorkerMetricsSnapshotRequest);
    });
  } catch (error) {
    return { error: formatMetricsError(error) };
  }
};

const captureProcessSnapshot = (metricsRuntime: MetricsRuntime) => {
  const captureStartedAt = metricsTimestamp();
  const snapshot = {
    processCpuUsageMicros: process.cpuUsage(),
    mainThreadCpuUsageMicros: process.threadCpuUsage(),
    processResourceUsage: process.resourceUsage(),
    processMemoryUsageBytes: process.memoryUsage(),
    mainIsolateHeapStatistics: metricsRuntime.getHeapStatistics(),
    mainEventLoopUtilization: metricsRuntime.performance.eventLoopUtilization(),
    mainIsolateGc: metricsRuntime.gcMetrics.snapshot(),
  };
  const captureFinishedAt = metricsTimestamp();
  return {
    capturedAt: captureStartedAt,
    captureStartedAt,
    captureFinishedAt,
    scope: {
      endpoints:
        'every resource read occurs synchronously between captureStartedAt and captureFinishedAt; Node.js does not expose each exact counter-read instant',
      cpuUsage: 'whole process, including JS workers and native threads',
      memoryUsage:
        'RSS is whole process; other process.memoryUsage fields follow main-thread/isolate semantics; RSS is not assigned to a worker',
      heapStatistics: 'main V8 isolate only',
      eventLoopUtilization: 'Node.js main event loop only; this is not CPU time',
    },
    ...snapshot,
  };
};

const createCpuWindowDiagnostic = ({
  phase,
  outerStart,
  outerEnd,
  innerStart,
  innerEnd,
  workers,
  workerEnds,
}: {
  phase: string;
  outerStart: ProcessSnapshot;
  outerEnd: ProcessSnapshot;
  innerStart?: ProcessSnapshot;
  innerEnd?: ProcessSnapshot;
  workers: InitializedWorker[];
  workerEnds?: Array<WorkerResourceCapture>;
}) => {
  const workerSamples = workers.map(
    ({ threadNumber, mainTimeline, resourcesAtPoolReady }, index) => {
      if (!resourcesAtPoolReady?.ok) {
        return { threadNumber, ok: false, error: resourcesAtPoolReady?.error ?? 'missing sample' };
      }
      const end = workerEnds?.[index];
      if (workerEnds && !end?.ok) {
        return { threadNumber, ok: false, error: end?.error ?? 'missing end sample' };
      }
      return {
        threadNumber,
        ok: true,
        measurementClass:
          workerEnds === undefined
            ? 'cumulative worker-thread CPU since an unknown point between constructor start and online, read asynchronously during the ready capture interval'
            : 'worker-thread CPU difference between two asynchronously completed capture intervals',
        relationToProcessWindows:
          workerEnds === undefined
            ? 'the worker CPU interval is contained by the outer process window, but its exact start and read instants are not exposed by Node.js'
            : 'the worker CPU interval is contained by the outer process window and contains the inner process window; it is neither an exact match for either process window nor exact plugin attribution',
        startBounds:
          workerEnds === undefined
            ? {
                earliestAt: mainTimeline.constructorStartedAt,
                latestAt: mainTimeline.onlineAt,
                meaning: 'Node.js does not expose the exact Worker.cpuUsage counter start instant',
              }
            : {
                earliestAt: resourcesAtPoolReady.snapshot.captureStartedAt,
                latestAt: resourcesAtPoolReady.snapshot.captureFinishedAt,
                meaning:
                  'the first asynchronous Worker.cpuUsage read completes within these bounds',
              },
        endBounds: {
          earliestAt:
            end?.ok === true
              ? end.snapshot.captureStartedAt
              : resourcesAtPoolReady.snapshot.captureStartedAt,
          latestAt:
            end?.ok === true
              ? end.snapshot.captureFinishedAt
              : resourcesAtPoolReady.snapshot.captureFinishedAt,
          meaning: 'the asynchronous Worker.cpuUsage read completes within these bounds',
        },
        cpuDeltaMicros:
          end?.ok === true
            ? subtractCpuUsage(
                end.snapshot.cpuUsageMicros,
                resourcesAtPoolReady.snapshot.cpuUsageMicros,
              )
            : resourcesAtPoolReady.snapshot.cpuUsageMicros,
      };
    },
  );
  const observedWorkerCpu = workerSamples.reduce<CpuUsageMicros>(
    (sum, sample) => {
      const cpuDelta = sample.cpuDeltaMicros;
      return {
        user: sum.user + (cpuDelta?.user ?? 0),
        system: sum.system + (cpuDelta?.system ?? 0),
      };
    },
    { user: 0, system: 0 },
  );
  return {
    measurementClass: 'asynchronous-bracketing-diagnostic; not exact CPU attribution',
    phase,
    outerProcessWindow: cpuProcessWindow(outerStart, outerEnd),
    ...(innerStart && innerEnd
      ? { innerProcessWindow: cpuProcessWindow(innerStart, innerEnd) }
      : {}),
    workerSamples,
    summedObservedWorkerThreadCpuMicros: observedWorkerCpu,
    completeWorkerCoverage: workerSamples.every(({ ok }) => ok),
    scope:
      'process and main-thread counters are read within synchronous process-snapshot capture bounds; worker CPU reads have different asynchronous bounds, so they are reported independently and are never subtracted into a claimed Rust/native residual',
  };
};

const cpuProcessWindow = (start: ProcessSnapshot, end: ProcessSnapshot) => ({
  measurementClass:
    'synchronous snapshot-bracketed cumulative-counter difference; exact CPU counter read instants are not exposed',
  startedAt: start.capturedAt,
  finishedAt: end.capturedAt,
  captureBounds: {
    start: {
      earliestAt: start.captureStartedAt,
      latestAt: start.captureFinishedAt,
      meaning: 'the start CPU counters are read synchronously within this interval',
    },
    end: {
      earliestAt: end.captureStartedAt,
      latestAt: end.captureFinishedAt,
      meaning: 'the end CPU counters are read synchronously within this interval',
    },
  },
  processCpuDeltaMicros: subtractCpuUsage(end.processCpuUsageMicros, start.processCpuUsageMicros),
  mainThreadCpuDeltaMicros: subtractCpuUsage(
    end.mainThreadCpuUsageMicros,
    start.mainThreadCpuUsageMicros,
  ),
  scope:
    'process CPU includes all JavaScript workers and native threads; main-thread CPU covers the parent Node.js thread; neither delta is plugin or native ownership',
});

const subtractCpuUsage = (end: CpuUsageMicros, start: CpuUsageMicros): CpuUsageMicros => ({
  user: end.user - start.user,
  system: end.system - start.system,
});

const formatMetricsError = (error: unknown) =>
  error instanceof Error ? `${error.name}: ${error.message}` : String(error);

const withMetricsTimeout = async <T>(promise: Promise<T>, label: string): Promise<T> => {
  let timeout: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_resolve, reject) => {
        timeout = setTimeout(() => reject(new Error(`timed out waiting for ${label}`)), 10_000);
      }),
    ]);
  } finally {
    if (timeout) clearTimeout(timeout);
  }
};

const availableParallelism = () => {
  // Research-only control for reproducible ParallelPlugin measurements.
  // This environment variable is not a public API.
  const configuredCount = process.env.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  if (configuredCount !== undefined) {
    const count = Number(configuredCount);
    if (!Number.isSafeInteger(count) || count < 1 || count > 64) {
      throw new Error('ROLLDOWN_PARALLEL_PLUGIN_WORKERS must be an integer from 1 to 64');
    }
    return count;
  }

  let availableParallelism = 1;
  try {
    availableParallelism = os.availableParallelism();
  } catch {
    const cpus = os.cpus();
    if (Array.isArray(cpus) && cpus.length > 0) {
      availableParallelism = cpus.length;
    }
  }
  return Math.min(availableParallelism, 8);
};

const writeMetrics = async (kind: string, fields: Record<string, unknown>) => {
  const report = { kind, version: 1, ...fields };
  validateParallelPluginLifecycleMetrics(report);
  await writeMetricsRecord('rolldown-parallel-plugin-init-metrics', report);
};

const writePostCloseMetrics = async (report: Record<string, unknown>) => {
  validateParallelPluginPostCloseMetrics(report);
  await writeMetricsRecord('rolldown-parallel-plugin-post-close-metrics', report);
};

const writeMetricsRecord = async (prefix: string, report: Record<string, unknown>) => {
  const record = `[${prefix}] ${JSON.stringify(report)}\n`;
  await new Promise<void>((resolve, reject) => {
    process.stderr.write(record, (error) => (error ? reject(error) : resolve()));
  });
};
