import { parentPort, workerData } from 'node:worker_threads';
import { registerPlugins } from './binding.cjs';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { defineParallelPluginImplementation } from './parallel-plugin';
import { bindingifyPlugin } from './plugin/bindingify-plugin';
import { PluginContextData } from './plugin/plugin-context-data';
import type { WorkerData } from './utils/initialize-parallel-plugins';

const { registryId, pluginInfos, threadNumber } = workerData as WorkerData;
(async () => {
  try {
    const plugins = await Promise.all(
      pluginInfos.map(async (pluginInfo) => {
        const pluginModule = await import(pluginInfo.fileUrl);
        const definePluginImpl = pluginModule.default as ReturnType<
          typeof defineParallelPluginImplementation
        >;
        const plugin = await definePluginImpl(pluginInfo.options, {
          threadNumber,
        });
        return {
          index: pluginInfo.index,
          // TODO(sapphi-red): support inputOptions and outputOptions
          plugin: bindingifyPlugin(
            plugin,
            {} as InputOptions,
            {} as OutputOptions,
            // TODO need to find a way to share pluginContextData
            new PluginContextData(() => {}, {} as OutputOptions, []),
            [],
            () => {},
            'info' as const,
            // TODO: support this.meta.watchMode
            false,
          ),
        };
      }),
    );

    registerPlugins(registryId, plugins);

    parentPort!.postMessage({ type: 'success' });
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
  } finally {
    parentPort!.unref();
  }
})();
