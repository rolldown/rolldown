import type { BindingBundlerOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { getLogger, getOnLog } from '../log/logger';
import { LOG_LEVEL_INFO } from '../log/logging';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { PluginContextData } from '../plugin/plugin-context-data';
import { PluginDriver } from '../plugin/plugin-driver';
import { getObjectPlugins } from '../plugin/plugin-driver';
import {
  assertParallelPluginOptionsSupported,
  assertParallelPluginsSupported,
} from '../plugin/parallel-plugin';
import { bindingifyInputOptions } from './bindingify-input-options';
import { bindingifyOutputOptions } from './bindingify-output-options';
import { initializeParallelPlugins } from './initialize-parallel-plugins';
import {
  createCleanupFailureError,
  isCleanupFailureError,
  retryCleanupFromError,
} from './retryable-cleanup';
import type { CloseCallbackScope } from './close-callback-scope';
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
  closeCallbackScope?: CloseCallbackScope,
): Promise<BundlerOptionWithStopWorker> {
  assertParallelPluginOptionsSupported(inputOptions.plugins, outputOptions.plugins);
  const inputPlugins = await normalizePluginOption(inputOptions.plugins, closeCallbackScope);
  const outputPlugins = await normalizePluginOption(outputOptions.plugins, closeCallbackScope);

  const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO;
  const onLog = getLogger(
    getObjectPlugins(inputPlugins),
    getOnLog(inputOptions, logLevel),
    logLevel,
    watchMode,
  );

  // The `outputOptions` hook is called with the input plugins and the output plugins
  const callOutputOptionsHook = () =>
    PluginDriver.callOutputOptionsHook(
      [...inputPlugins, ...outputPlugins],
      outputOptions,
      onLog,
      logLevel,
      watchMode,
    );
  outputOptions = closeCallbackScope
    ? closeCallbackScope.run(callOutputOptionsHook)
    : callOutputOptionsHook();

  assertParallelPluginOptionsSupported(outputOptions.plugins);
  const hookOutputPlugins = await normalizePluginOption(outputOptions.plugins, closeCallbackScope);
  const normalizedInputPlugins = normalizePlugins(inputPlugins, ANONYMOUS_PLUGIN_PREFIX);
  const normalizedOutputPlugins = normalizePlugins(
    hookOutputPlugins,
    ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  );

  let plugins = [
    ...normalizedInputPlugins,
    ...checkOutputPluginOption(normalizedOutputPlugins, onLog),
  ];

  let parallelPluginInitResult: Awaited<ReturnType<typeof initializeParallelPlugins>>;
  try {
    if (import.meta.browserBuild) {
      if (plugins.some((plugin) => '_parallel' in plugin)) {
        assertParallelPluginsSupported();
      }
      parallelPluginInitResult = undefined;
    } else {
      parallelPluginInitResult = await initializeParallelPlugins(plugins);
    }
  } catch (error) {
    if (!isCleanupFailureError(error)) throw error;
    return retryCleanupFromError(
      error,
      'Parallel-plugin worker initialization and retry cleanup both failed',
    );
  }

  try {
    // Warn if deprecated experimental.strictExecutionOrder is used
    if ((inputOptions.experimental as any)?.strictExecutionOrder !== undefined) {
      console.warn(
        '`experimental.strictExecutionOrder` has been stabilized and moved to `output.strictExecutionOrder`. Please update your configuration.',
      );
    }

    const pluginContextData = new PluginContextData(
      onLog,
      outputOptions,
      normalizedInputPlugins,
      normalizedOutputPlugins,
    );

    // Convert `InputOptions` to `BindingInputOptions`
    const bindingInputOptions = bindingifyInputOptions(
      plugins,
      inputOptions,
      outputOptions,
      pluginContextData,
      normalizedOutputPlugins,
      onLog,
      logLevel,
      watchMode,
    );

    // Convert `OutputOptions` to `BindingOutputOptions`
    const bindingOutputOptions = bindingifyOutputOptions(outputOptions, pluginContextData);

    const bundlerOptions: BindingBundlerOptions = {
      inputOptions: bindingInputOptions,
      outputOptions: bindingOutputOptions,
      parallelPluginsRegistry: parallelPluginInitResult?.registry,
    };

    return {
      bundlerOptions: closeCallbackScope
        ? closeCallbackScope.wrapCallbacks(bundlerOptions)
        : bundlerOptions,
      inputOptions,
      onLog,
      stopWorkers: parallelPluginInitResult?.stopWorkers,
    };
  } catch (error) {
    const stopWorkers = parallelPluginInitResult?.stopWorkers;
    if (!stopWorkers) throw error;
    try {
      await stopWorkers();
    } catch (cleanupError) {
      return retryCleanupFromError(
        createCleanupFailureError(
          error,
          cleanupError,
          stopWorkers,
          'Bundler option setup and parallel-plugin worker cleanup both failed',
        ),
        'Bundler option setup and parallel-plugin worker retry cleanup both failed',
      );
    }
    throw error;
  }
}

export interface BundlerOptionWithStopWorker {
  bundlerOptions: BindingBundlerOptions;
  inputOptions: InputOptions;
  onLog: LogHandler;
  stopWorkers?: () => Promise<void>;
}
