import { parentPort, workerData } from 'node:worker_threads';
import { registerPlugins } from './binding.cjs';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { defineParallelPluginImplementation } from './parallel-plugin';
import { bindingifyPlugin } from './plugin/bindingify-plugin';
import { PluginContextData } from './plugin/plugin-context-data';
import type { WorkerData } from './utils/initialize-parallel-plugins';

const { registryId, pluginInfos, threadNumber, metricsEnabled } = workerData as WorkerData;
(async () => {
  try {
    if (!metricsEnabled) {
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
    } else {
      const bootstrapStartedAt = performance.now();
      const initializedPlugins = await Promise.all(
        pluginInfos.map(async (pluginInfo) => {
          const importStartedAt = performance.now();
          const pluginModule = await import(pluginInfo.fileUrl);
          const importFinishedAt = performance.now();
          const definePluginImpl = pluginModule.default as ReturnType<
            typeof defineParallelPluginImplementation
          >;
          const factoryStartedAt = performance.now();
          const plugin = await definePluginImpl(pluginInfo.options, {
            threadNumber,
          });
          const factoryFinishedAt = performance.now();
          const bindingStartedAt = performance.now();
          const bindingPlugin = bindingifyPlugin(
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
          );
          const bindingFinishedAt = performance.now();
          return {
            registration: { index: pluginInfo.index, plugin: bindingPlugin },
            metrics: {
              pluginIndex: pluginInfo.index,
              implementationImportMs: importFinishedAt - importStartedAt,
              factoryMs: factoryFinishedAt - factoryStartedAt,
              bindingifyMs: bindingFinishedAt - bindingStartedAt,
            },
          };
        }),
      );

      const registerStartedAt = performance.now();
      registerPlugins(
        registryId,
        initializedPlugins.map(({ registration }) => registration),
      );
      const registerFinishedAt = performance.now();

      parentPort!.postMessage({
        type: 'success',
        metrics: {
          measuredBootstrapMs: registerFinishedAt - bootstrapStartedAt,
          registerPluginsMs: registerFinishedAt - registerStartedAt,
          plugins: initializedPlugins.map(({ metrics }) => metrics),
        },
      });
    }
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
    parentPort!.unref();
    return;
  }
  // Hold the worker alive so Rust can dispatch plugin hook callbacks through
  // the thread-safe functions registered during bootstrap. The main thread
  // terminates the worker explicitly when the build completes.
  setInterval(() => {}, 1 << 30);
})();
