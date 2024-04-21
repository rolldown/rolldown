import { Worker } from 'node:worker_threads'
import { availableParallelism } from 'node:os'
import type { ParallelPlugin, Plugin } from '../plugin'
import { ParallelJsPluginRegistry } from '../binding'

export type WorkerData = {
  registryId: number
  pluginInfos: ParallelPluginInfo[]
  threadNumber: number
}

type ParallelPluginInfo = {
  index: number
  fileUrl: string
  options: unknown
}

export async function initializeParallelPlugins(
  plugins: (Plugin | ParallelPlugin)[],
) {
  const pluginInfos: ParallelPluginInfo[] = []
  for (const [index, plugin] of plugins.entries()) {
    if ('_parallel' in plugin) {
      const { fileUrl, options } = plugin._parallel
      pluginInfos.push({ index, fileUrl, options })
    }
  }
  if (pluginInfos.length <= 0) {
    return undefined
  }

  const count = Math.min(availableParallelism(), 8)
  const parallelJsPluginRegistry = new ParallelJsPluginRegistry(count)
  const registryId = parallelJsPluginRegistry.id

  const workers = await initializeWorkers(registryId, count, pluginInfos)
  const stopWorkers = async () => {
    await Promise.all(workers.map((worker) => worker.terminate()))
  }

  return { registry: parallelJsPluginRegistry, stopWorkers }
}

export function initializeWorkers(
  registryId: number,
  count: number,
  pluginInfos: ParallelPluginInfo[],
) {
  return Promise.all(
    Array.from({ length: count }, (_, i) =>
      initializeWorker(registryId, pluginInfos, i),
    ),
  )
}

async function initializeWorker(
  registryId: number,
  pluginInfos: ParallelPluginInfo[],
  threadNumber: number,
) {
  const urlString = import.meta.resolve('#parallel-plugin-worker')
  const worker = new Worker(new URL(urlString), {
    workerData: { registryId, pluginInfos, threadNumber } satisfies WorkerData,
  })
  worker.unref()
  await new Promise<void>((resolve) => {
    worker.once('message', async () => {
      resolve()
    })
  })
  return worker
}
