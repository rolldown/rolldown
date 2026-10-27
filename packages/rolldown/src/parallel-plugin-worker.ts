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
        // TODO need to find a way to share pluginContextData
        const pluginContextData = new PluginContextData(() => {}, {} as OutputOptions, [], []);
        // Every plugin here has its own context and each hook call goes to whichever worker is
        // free, so a `chunk.code` cache never hits in a worker. It would only keep the chunks of
        // every build alive, because nothing clears it between watch rebuilds.
        pluginContextData.outputCodeCache = null;
        return {
          index: pluginInfo.index,
          // TODO(sapphi-red): support inputOptions and outputOptions
          plugin: bindingifyPlugin(
            plugin,
            {} as InputOptions,
            {} as OutputOptions,
            pluginContextData,
            [],
            () => {},
            'info' as const,
            // TODO: support this.meta.watchMode
            false,
            // Not measured: these hooks run on a worker thread, so their spans could not
            // be summed with the main thread's or reported through its log handler.
            undefined,
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
