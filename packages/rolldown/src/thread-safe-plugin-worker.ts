import { parentPort, workerData } from 'node:worker_threads'
import { registerPlugins } from './binding'
import type { WorkerData } from './utils/initialize-thread-safe-plugins'
import type { defineThreadSafePluginImplementation } from './thread-safe-plugin'
import { bindingifyPlugin } from './plugin/bindingify-plugin'
import { RolldownNormalizedInputOptions } from './options/input-options'

const { registryId, pluginInfos, threadNumber } = workerData as WorkerData

;(async () => {
  // TODO: handle error
  const plugins = await Promise.all(
    pluginInfos.map(async (pluginInfo) => {
      const pluginModule = await import(pluginInfo.fileUrl)
      const definePluginImpl = pluginModule.default as ReturnType<
        typeof defineThreadSafePluginImplementation
      >
      const plugin = await definePluginImpl(pluginInfo.options, {
        threadNumber,
      })
      return {
        index: pluginInfo.index,
        // TODO: support inputOptions
        plugin: bindingifyPlugin(plugin, {} as RolldownNormalizedInputOptions),
      }
    }),
  )

  registerPlugins(registryId, plugins)

  parentPort!.postMessage('')
  parentPort!.unref()
})()
