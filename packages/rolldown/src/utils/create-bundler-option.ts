import type { BindingBundlerOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { getLogger, getOnLog } from '../log/logger';
import { LOG_LEVEL_INFO } from '../log/logging';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import { PluginContextData } from '../plugin/plugin-context-data';
import { PluginDriver } from '../plugin/plugin-driver';
import { getObjectPlugins } from '../plugin/plugin-driver';
import { bindingifyInputOptions } from './bindingify-input-options';
import { bindingifyOutputOptions } from './bindingify-output-options';
import { initializeParallelPlugins } from './initialize-parallel-plugins';
import {
  measureIfFunction,
  OUTPUT_OPTIONS_OWNER,
  pluginTimingsRecorderFor,
} from './plugin-timings';
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
  /**
   * Whether to time plugin hooks. Only `RolldownBuild` asks for it, because it is the one
   * caller that reaches `close()`, where the report is flushed. A recorder created for
   * watch, dev or scan would be written to on every hook call and never read.
   */
  measureTimings = false,
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

  // Keyed on the input options so a plugin running a nested `rolldown()` build accumulates
  // separately from the build that spawned it, and so repeated `generate`/`write` calls on
  // one build share a recorder — `close()` flushes it once, keyed on the same object.
  const timings =
    measureTimings && inputOptions.checks?.pluginTimings !== false
      ? pluginTimingsRecorderFor(inputOptions)
      : undefined;

  // `assetFileNames` and `sanitizeFileName` are read twice: once here on the way to Rust,
  // and again by `this.emitFile` in a plugin context, which calls the user's option
  // directly. Measuring at each consumer would count one call in two places, so these two
  // are measured once at the source and passed on already wrapped.
  if (timings) {
    outputOptions = {
      ...outputOptions,
      assetFileNames: measureIfFunction(
        timings,
        OUTPUT_OPTIONS_OWNER,
        'assetFileNames',
        outputOptions.assetFileNames,
      ),
      sanitizeFileName: measureIfFunction(
        timings,
        OUTPUT_OPTIONS_OWNER,
        'sanitizeFileName',
        outputOptions.sanitizeFileName,
      ),
    };
  }

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
      timings,
    );

    // Convert `OutputOptions` to `BindingOutputOptions`
    const bindingOutputOptions = bindingifyOutputOptions(outputOptions, pluginContextData, timings);

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
