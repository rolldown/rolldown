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
            new PluginContextData(() => {}, {} as OutputOptions, [], []),
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
    parentPort!.unref();
    return;
  }
  // Hold the worker alive (poll-style) so the TSFNs that wrap each plugin
  // hook can be dispatched. The main thread terminates each worker explicitly
  // via `worker.terminate()` when the build completes. Required on Node 24.x:
  // without this the worker's JS event loop exits as soon as bootstrap
  // returns, and the first hook dispatch from the main thread gets
  // `Status::Closing`. Reproduces without this patch in
  // `examples/par-plugin/parallel-noop-plugin/`.
  setInterval(() => {}, 1 << 30);
})();
