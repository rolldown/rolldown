import os from 'node:os';
import type { performance as NodePerformance } from 'node:perf_hooks';
import type { getHeapStatistics as getNodeHeapStatistics } from 'node:v8';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';

export type WorkerData = {
  registryId: number;
  pluginInfos: ParallelPluginInfo[];
  threadNumber: number;
  metricsEnabled?: true;
  metricsMainTimeOriginEpochMs?: number;
};

export type MetricsTimestamp = {
  monotonicMs: number;
  epochMs: number;
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

type MainMetricsRuntime = {
  performance: typeof NodePerformance;
  getHeapStatistics: typeof getNodeHeapStatistics;
};

let nextMetricsRequestId = 1;

export async function initializeParallelPlugins(plugins: RolldownPlugin[]): Promise<
  | {
      registry: ParallelJsPluginRegistry;
      stopWorkers: () => Promise<void>;
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

  const metricsEnabled = process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS === 'json';
  const metricsRuntime = metricsEnabled ? await createMainMetricsRuntime() : undefined;
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

  const initializedWorkers = await initializeWorkersWithMetrics(registryId, count, pluginInfos);
  const workers = initializedWorkers.map(({ worker }) => worker);
  const poolInitializationMs = performance.now() - initializationStartedAt;
  const processWhenAllWorkersReady = captureProcessSnapshot(metricsRuntime!);
  const resourcesAtPoolReady = await Promise.all(
    initializedWorkers.map(({ worker }) => captureWorkerResources(worker)),
  );
  for (const [index, resources] of resourcesAtPoolReady.entries()) {
    initializedWorkers[index].resourcesAtPoolReady = resources;
  }
  const processAtPoolReady = captureProcessSnapshot(metricsRuntime!);
  writeMetrics('rolldown_parallel_plugin_init_metrics', {
    workerCount: count,
    pluginCount: pluginInfos.length,
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
    cpuAttribution: calculateCpuAttribution(
      processBeforeWorkerPool!,
      processAtPoolReady,
      initializedWorkers.map(({ resourcesAtPoolReady }) => resourcesAtPoolReady),
      undefined,
    ),
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
  const stopWorkers = async () => {
    const terminationStartedAt = performance.now();
    const processBeforeWorkerSnapshots = captureProcessSnapshot(metricsRuntime!);
    const beforeTermination = await Promise.all(
      initializedWorkers.map(({ worker }) => captureWorkerBeforeTermination(worker)),
    );
    const processAfterWorkerSnapshots = captureProcessSnapshot(metricsRuntime!);
    await Promise.all(workers.map((worker) => worker.terminate()));
    const processAfterTermination = captureProcessSnapshot(metricsRuntime!);
    writeMetrics('rolldown_parallel_plugin_termination_metrics', {
      workerCount: count,
      poolTerminationMs: performance.now() - terminationStartedAt,
      rssBeforeBytes: processBeforeWorkerSnapshots.processMemoryUsageBytes.rss,
      rssAfterBytes: processAfterTermination.processMemoryUsageBytes.rss,
      rssScope: 'whole process; the before/after delta is not worker ownership',
      processSnapshots: {
        scope: 'whole process; RSS is not attributed to an isolate or worker',
        resourceBaselineBeforeBuild: processAtPoolReady,
        beforeWorkerSnapshots: processBeforeWorkerSnapshots,
        afterWorkerSnapshots: processAfterWorkerSnapshots,
        afterTermination: processAfterTermination,
      },
      cpuAttribution: calculateCpuAttribution(
        processAtPoolReady,
        processAfterWorkerSnapshots,
        initializedWorkers.map(({ resourcesAtPoolReady }) => resourcesAtPoolReady),
        beforeTermination.map(({ resources }) => resources),
      ),
      workers: initializedWorkers.map(({ threadNumber, resourcesAtPoolReady }, index) => ({
        threadNumber,
        resourcesAtPoolReady,
        resourcesBeforeTermination: beforeTermination[index].resources,
        workerLocalBeforeTermination: beforeTermination[index].workerLocal,
      })),
    });
  };

  return { registry: parallelJsPluginRegistry, stopWorkers };
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
): Promise<InitializedWorker[]> {
  const results = await Promise.allSettled(
    Array.from({ length: count }, (_, i) =>
      initializeWorkerWithMetrics(registryId, pluginInfos, i),
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
) {
  const urlString = import.meta.resolve('#parallel-plugin-worker');
  const workerData: WorkerData = {
    registryId,
    pluginInfos,
    threadNumber,
    metricsEnabled: true,
    metricsMainTimeOriginEpochMs: performance.timeOrigin,
  };

  let worker: Worker | undefined;
  try {
    const constructorStartedAt = metricsTimestamp();
    worker = new Worker(new URL(urlString), { workerData });
    const constructorReturnedAt = metricsTimestamp();
    const { onlineAt, message } = await waitForWorkerReady(worker);
    const readyMessageAt = metricsTimestamp();
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

const captureProcessSnapshot = (metricsRuntime: MainMetricsRuntime) => ({
  capturedAt: metricsTimestamp(),
  scope: {
    cpuUsage: 'whole process, including JS workers and native threads',
    memoryUsage: 'whole process; RSS is not assigned to a worker',
    heapStatistics: 'main V8 isolate only',
    eventLoopUtilization: 'Node.js main event loop only; this is not CPU time',
  },
  processCpuUsageMicros: process.cpuUsage(),
  processResourceUsage: process.resourceUsage(),
  processMemoryUsageBytes: process.memoryUsage(),
  mainIsolateHeapStatistics: metricsRuntime.getHeapStatistics(),
  mainEventLoopUtilization: metricsRuntime.performance.eventLoopUtilization(),
});

const calculateCpuAttribution = (
  processStart: ProcessSnapshot,
  processEnd: ProcessSnapshot,
  workerStarts: Array<WorkerResourceCapture | undefined>,
  workerEnds: Array<WorkerResourceCapture | undefined> | undefined,
) => {
  const processDelta = subtractCpuUsage(
    processEnd.processCpuUsageMicros,
    processStart.processCpuUsageMicros,
  );
  const workerDeltas = workerStarts.map((start, index) => {
    if (!start?.ok) {
      return undefined;
    }
    if (workerEnds === undefined) {
      return start.snapshot.cpuUsageMicros;
    }
    const end = workerEnds[index];
    return end?.ok
      ? subtractCpuUsage(end.snapshot.cpuUsageMicros, start.snapshot.cpuUsageMicros)
      : undefined;
  });
  const measuredWorkerCpu = workerDeltas.reduce<CpuUsageMicros>(
    (sum, delta) => ({
      user: sum.user + (delta?.user ?? 0),
      system: sum.system + (delta?.system ?? 0),
    }),
    { user: 0, system: 0 },
  );
  return {
    processCpuDeltaMicros: processDelta,
    measuredWorkerCpuDeltaMicros: measuredWorkerCpu,
    measuredWorkerThreadCpuDeltaMicros: measuredWorkerCpu,
    residualProcessCpuDeltaMicros: subtractCpuUsage(processDelta, measuredWorkerCpu),
    completeWorkerCoverage: workerDeltas.every((delta) => delta !== undefined),
    workerCpuScope:
      'Worker.cpuUsage measures each Node.js worker thread, including V8, garbage collection, runtime, and native work executed on that thread; it excludes helper threads and is not pure plugin JavaScript CPU',
    residualMeaning:
      'process CPU minus measured Node.js worker-thread CPU; includes the Node.js main thread, Rolldown/Rust/native threads, runtime helper threads, measurement skew, and any unmeasured worker CPU',
  };
};

const subtractCpuUsage = (end: CpuUsageMicros, start: CpuUsageMicros): CpuUsageMicros => ({
  user: end.user - start.user,
  system: end.system - start.system,
});

const metricsTimestamp = (): MetricsTimestamp => {
  const monotonicMs = performance.now();
  return { monotonicMs, epochMs: performance.timeOrigin + monotonicMs };
};

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

const createMainMetricsRuntime = async (): Promise<MainMetricsRuntime> => {
  const [{ performance }, { getHeapStatistics }] = await Promise.all([
    import('node:perf_hooks'),
    import('node:v8'),
  ]);
  return { performance, getHeapStatistics };
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

const writeMetrics = (kind: string, fields: Record<string, unknown>) => {
  process.stderr.write(
    `[rolldown-parallel-plugin-init-metrics] ${JSON.stringify({ kind, version: 1, ...fields })}\n`,
  );
};
