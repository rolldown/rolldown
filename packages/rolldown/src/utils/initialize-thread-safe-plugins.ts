import { Worker } from 'node:worker_threads'
import { availableParallelism } from 'node:os'
import { ThreadSafePlugin, Plugin } from '../plugin'
import { ThreadSafePluginRegistry } from '../binding'

export type WorkerData = {
  registryId: number
  pluginInfos: ThreadSafePluginInfo[]
  threadNumber: number
}

type ThreadSafePluginInfo = {
  index: number
  fileUrl: string
  options: unknown
}

export async function initializeThreadSafePlugins(
  plugins: (Plugin | ThreadSafePlugin)[],
) {
  const pluginInfos: ThreadSafePluginInfo[] = []
  for (const [index, plugin] of plugins.entries()) {
    if ('_threadSafe' in plugin) {
      const { fileUrl, options } = plugin._threadSafe
      pluginInfos.push({ index, fileUrl, options })
    }
  }
  if (pluginInfos.length <= 0) {
    return undefined
  }

  const count = Math.min(availableParallelism(), 8)
  const threadSafePluginRegistry = new ThreadSafePluginRegistry(count)
  const registryId = threadSafePluginRegistry.id

  const workers = await initializeWorkers(registryId, count, pluginInfos)
  const stopWorkers = async () => {
    await Promise.all(workers.map((worker) => worker.terminate()))
  }

  return { registry: threadSafePluginRegistry, stopWorkers }
}

export function initializeWorkers(
  registryId: number,
  count: number,
  pluginInfos: ThreadSafePluginInfo[],
) {
  return Promise.all(
    Array.from({ length: count }, (_, i) =>
      initializeWorker(registryId, pluginInfos, i),
    ),
  )
}

async function initializeWorker(
  registryId: number,
  pluginInfos: ThreadSafePluginInfo[],
  threadNumber: number,
) {
  const urlString = import.meta.resolve('#thread-safe-plugin-worker')
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
