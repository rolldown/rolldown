import { parentPort, workerData } from 'node:worker_threads'
import { registerPlugins } from './binding'
import type { WorkerData } from './utils/initialize-parallel-plugins'
import type { defineParallelPluginImplementation } from './parallel-plugin'
import { bindingifyPlugin } from './plugin/bindingify-plugin'
import type { NormalizedInputOptions } from './options/normalized-input-options'
import type { NormalizedOutputOptions } from './options/normalized-output-options'
import { PluginContextData } from './plugin/plugin-context-data'

const { registryId, pluginInfos, threadNumber } = workerData as WorkerData

;(async () => {
  try {
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
            {} as NormalizedInputOptions,
            {} as NormalizedOutputOptions,
            // TODO need to find a way to share pluginContextData
            new PluginContextData(),
          ),
        }
      }),
    )

    registerPlugins(registryId, plugins)

    parentPort!.postMessage({ type: 'success' })
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error })
  } finally {
    parentPort!.unref()
  }
})()
