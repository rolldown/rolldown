import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding.cjs';
import type { RolldownPlugin } from '../plugin';

export type WorkerData = {
  registryId: number;
  pluginInfos: ParallelPluginInfo[];
  threadNumber: number;
};

type ParallelPluginInfo = {
  index: number;
  fileUrl: string;
  options: unknown;
};

export interface TerminableWorker {
  terminate(): Promise<number>;
}

/** @internal Retry only workers whose previous termination attempt failed. */
export async function terminateWorkersWithRetry<T extends TerminableWorker>(
  workers: T[],
  maxAttempts: number,
): Promise<{ errors: unknown[]; remainingWorkers: T[] }> {
  let remainingWorkers = workers;
  let errors: unknown[] = [];
  for (let attempt = 0; attempt < maxAttempts && remainingWorkers.length > 0; attempt += 1) {
    const currentWorkers = remainingWorkers;
    const results = await Promise.allSettled(currentWorkers.map((worker) => worker.terminate()));
    remainingWorkers = currentWorkers.filter((_, index) => results[index].status === 'rejected');
    errors = results.flatMap((result) => (result.status === 'rejected' ? [result.reason] : []));
  }
  return { errors, remainingWorkers };
}

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

  const count = availableParallelism();
  const parallelJsPluginRegistry = new ParallelJsPluginRegistry(count);
  const registryId = parallelJsPluginRegistry.id;

  let workers = await initializeWorkers(registryId, count, pluginInfos);
  const stopWorkers = async () => {
    const result = await terminateWorkersWithRetry(workers, 1);
    workers = result.remainingWorkers;
    if (result.errors.length === 1) throw result.errors[0];
    if (result.errors.length > 1) {
      throw new AggregateError(result.errors, 'Parallel-plugin worker shutdown failed');
    }
  };

  return { registry: parallelJsPluginRegistry, stopWorkers };
}

function initializeWorkers(
  registryId: number,
  count: number,
  pluginInfos: ParallelPluginInfo[],
): Promise<Worker[]> {
  return Promise.all(
    Array.from({ length: count }, (_, i) => initializeWorker(registryId, pluginInfos, i)),
  );
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
    worker?.terminate();
    throw e;
  }
}

const availableParallelism = () => {
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
