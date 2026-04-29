import type { BindingBundlerOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { getLogger, getOnLog } from '../log/logger';
import { LOG_LEVEL_INFO } from '../log/logging';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { PluginDriver } from '../plugin/plugin-driver';
import { getObjectPlugins } from '../plugin/plugin-driver';
import { bindingifyInputOptions } from './bindingify-input-options';
import { bindingifyOutputOptions } from './bindingify-output-options';
import { initializeParallelPlugins } from './initialize-parallel-plugins';
import {
  ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  ANONYMOUS_PLUGIN_PREFIX,
  checkOutputPluginOption,
  normalizePluginOption,
  normalizePlugins,
} from './normalize-plugin-option';

export async function createBundlerOptions(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  watchMode: boolean,
): Promise<BundlerOptionWithStopWorker> {
  const inputPlugins = await normalizePluginOption(inputOptions.plugins);
  const outputPlugins = await normalizePluginOption(outputOptions.plugins);

  const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO;
  const onLog = getLogger(
    getObjectPlugins(inputPlugins),
    getOnLog(inputOptions, logLevel),
    logLevel,
    watchMode,
  );

  // The `outputOptions` hook is called with the input plugins and the output plugins
  outputOptions = PluginDriver.callOutputOptionsHook(
    [...inputPlugins, ...outputPlugins],
    outputOptions,
    onLog,
    logLevel,
    watchMode,
  );

  const hookOutputPlugins = await normalizePluginOption(outputOptions.plugins);
  const normalizedInputPlugins = normalizePlugins(inputPlugins, ANONYMOUS_PLUGIN_PREFIX);
  const normalizedOutputPlugins = normalizePlugins(
    hookOutputPlugins,
    ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  );

  let plugins = [
    ...normalizedInputPlugins,
    ...checkOutputPluginOption(normalizedOutputPlugins, onLog),
  ];

  const parallelPluginInitResult = import.meta.browserBuild
    ? undefined
    : await initializeParallelPlugins(plugins);

  // Warn if deprecated experimental.strictExecutionOrder is used
  if ((inputOptions.experimental as any)?.strictExecutionOrder !== undefined) {
    console.warn(
      '`experimental.strictExecutionOrder` has been stabilized and moved to `output.strictExecutionOrder`. Please update your configuration.',
    );
  }

  try {
    // Convert `InputOptions` to `BindingInputOptions`
    const { options: bindingInputOptions, pluginContextData } = bindingifyInputOptions(
      plugins,
      inputOptions,
      outputOptions,
      normalizedInputPlugins,
      normalizedOutputPlugins,
      onLog,
      logLevel,
      watchMode,
    );

    // Convert `OutputOptions` to `BindingOutputOptions`
    const bindingOutputOptions = bindingifyOutputOptions(outputOptions, pluginContextData);

    return {
      bundlerOptions: {
        inputOptions: bindingInputOptions,
        outputOptions: bindingOutputOptions,
        parallelPluginsRegistry: parallelPluginInitResult?.registry,
      },
      inputOptions,
      onLog,
      stopWorkers: parallelPluginInitResult?.stopWorkers,
    };
  } catch (e) {
    await parallelPluginInitResult?.stopWorkers();
    throw e;
  }
}

export interface BundlerOptionWithStopWorker {
  bundlerOptions: BindingBundlerOptions;
  inputOptions: InputOptions;
  onLog: LogHandler;
  stopWorkers?: () => Promise<void>;
}
