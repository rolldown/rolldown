import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { ParallelJsPluginRegistry } from '../binding';
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

export async function initializeParallelPlugins(
  plugins: RolldownPlugin[],
): Promise<
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

  const workers = await initializeWorkers(registryId, count, pluginInfos);
  const stopWorkers = async () => {
    await Promise.all(workers.map((worker) => worker.terminate()));
  };

  return { registry: parallelJsPluginRegistry, stopWorkers };
}

function initializeWorkers(
  registryId: number,
  count: number,
  pluginInfos: ParallelPluginInfo[],
): Promise<Worker[]> {
  return Promise.all(
    Array.from(
      { length: count },
      (_, i) => initializeWorker(registryId, pluginInfos, i),
    ),
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
