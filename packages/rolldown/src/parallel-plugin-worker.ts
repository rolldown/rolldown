import { parentPort, workerData } from 'node:worker_threads'
import { type BindingOutputOptions, registerPlugins } from './binding'
import type { WorkerData } from './utils/initialize-parallel-plugins'
import type { defineParallelPluginImplementation } from './parallel-plugin'
import { bindingifyPlugin } from './plugin/bindingify-plugin'
import type { RolldownNormalizedInputOptions } from './options/input-options'

const { registryId, pluginInfos, threadNumber } = workerData as WorkerData

;(async () => {
  // TODO(sapphi-red): handle error
  const plugins = await Promise.all(
    pluginInfos.map(async (pluginInfo) => {
      const pluginModule = await import(pluginInfo.fileUrl)
      const definePluginImpl = pluginModule.default as ReturnType<
        typeof defineParallelPluginImplementation
      >
      const plugin = await definePluginImpl(pluginInfo.options, {
        threadNumber,
      })
      return {
        index: pluginInfo.index,
        // TODO(sapphi-red): support inputOptions and outputOptions
        plugin: bindingifyPlugin(
          plugin,
          {} as RolldownNormalizedInputOptions,
          {} as BindingOutputOptions,
        ),
      }
    }),
  )

  registerPlugins(registryId, plugins)

  parentPort!.postMessage('')
  parentPort!.unref()
})()
