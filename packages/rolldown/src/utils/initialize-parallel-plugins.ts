import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';

export type WorkerData = {
  registryId: number;
  pluginInfos: ParallelPluginInfo[];
  threadNumber: number;
  metricsEnabled?: true;
};

type ParallelPluginInfo = {
  index: number;
  fileUrl: string;
  options: unknown;
};

type InitializedWorker = {
  worker: Worker;
  threadNumber: number;
  mainReadyMs: number;
  workerBootstrap: unknown;
};

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
  const initializationStartedAt = metricsEnabled ? performance.now() : 0;
  const rssBeforeBytes = metricsEnabled ? process.memoryUsage.rss() : 0;
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
  if (metricsEnabled) {
    writeMetrics('rolldown_parallel_plugin_init_metrics', {
      workerCount: count,
      pluginCount: pluginInfos.length,
      poolInitializationMs: performance.now() - initializationStartedAt,
      rssBeforeBytes,
      rssAfterBytes: process.memoryUsage.rss(),
      workers: initializedWorkers.map(({ threadNumber, mainReadyMs, workerBootstrap }) => ({
        threadNumber,
        mainReadyMs,
        workerBootstrap,
      })),
    });
  }
  const stopWorkers = async () => {
    const terminationStartedAt = metricsEnabled ? performance.now() : 0;
    const terminationRssBeforeBytes = metricsEnabled ? process.memoryUsage.rss() : 0;
    await Promise.all(workers.map((worker) => worker.terminate()));
    if (metricsEnabled) {
      writeMetrics('rolldown_parallel_plugin_termination_metrics', {
        workerCount: count,
        poolTerminationMs: performance.now() - terminationStartedAt,
        rssBeforeBytes: terminationRssBeforeBytes,
        rssAfterBytes: process.memoryUsage.rss(),
      });
    }
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
    worker.unref();
    await new Promise<void>((resolve, reject) => {
      worker!.once('message', async (message) => {
        if (message.type === 'error') {
          reject(message.error);
        } else {
          resolve();
        }
      });
    });
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
  };

  let worker: Worker | undefined;
  try {
    const startedAt = performance.now();
    worker = new Worker(new URL(urlString), { workerData });
    worker.unref();
    const message = await waitForWorker(worker);
    return {
      worker,
      threadNumber,
      mainReadyMs: performance.now() - startedAt,
      workerBootstrap: message.metrics,
    };
  } catch (e) {
    await worker?.terminate();
    throw e;
  }
}

const waitForWorker = (worker: Worker) =>
  new Promise<{ type: string; error?: unknown; metrics?: unknown }>((resolve, reject) => {
    worker.once('message', (message) => {
      if (message.type === 'error') {
        reject(message.error);
      } else {
        resolve(message);
      }
    });
  });

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
