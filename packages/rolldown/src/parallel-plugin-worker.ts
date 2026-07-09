// FIRST import: this worker env loads the binding, so it must register its
// own timer host (the per-env contract from timer-host.ts). On native the
// process-global driver registry can mask a missing registration (main's
// driver serves), but on the wasm artifacts the registry is per-instance --
// without this, a parallel-plugin worker's instance is genuinely driverless
// and a CurrentThread sleep there panics (Codex task-7 round 4, finding 3).
import './timer-host';
import { registerPlugins } from './binding.cjs';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { defineParallelPluginImplementation } from './parallel-plugin';
import { bindingifyPlugin } from './plugin/bindingify-plugin';
import { PluginContextData } from './plugin/plugin-context-data';
import type { ParallelPluginWorkerData } from './utils/initialize-parallel-plugins';

/**
 * @internal Invoked only after the lexical bootstrap has hidden its control
 * capability and every worker in the pool has reported binding readiness.
 */
export async function initializeParallelPluginWorker(
  workerData: ParallelPluginWorkerData,
): Promise<void> {
  const { registryId, pluginInfos, threadNumber, watchMode } = workerData;
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
          watchMode,
        ),
      };
    }),
  );

  registerPlugins(registryId, plugins);
}
