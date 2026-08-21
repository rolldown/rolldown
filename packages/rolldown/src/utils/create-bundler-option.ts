import type { BindingBundlerOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { getLogger, getOnLog } from '../log/logger';
import { LOG_LEVEL_INFO } from '../log/logging';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import type { Plugin } from '../plugin';
import { PluginContextData } from '../plugin/plugin-context-data';
import { PluginDriver } from '../plugin/plugin-driver';
import { getObjectPlugins } from '../plugin/plugin-driver';
import type { BuildCallbackRunner } from '../plugin/bindingify-plugin';
import {
  assertParallelPluginOptionsSupported,
  assertParallelPluginsSupported,
} from '../plugin/parallel-plugin';
import { bindingifyInputOptions } from './bindingify-input-options';
import { bindingifyOutputOptions } from './bindingify-output-options';
import type { CloseCallbackScope } from './close-callback-scope';
import { initializeParallelPlugins } from './initialize-parallel-plugins';
import {
  measureIfFunction,
  OUTPUT_OPTIONS_OWNER,
  pluginTimingsRecorderFor,
} from './plugin-timings';
import {
  createCleanupFailureError,
  isCleanupFailureError,
  retryCleanupFromError,
} from './retryable-cleanup';
import {
  ANONYMOUS_OUTPUT_PLUGIN_PREFIX,
  ANONYMOUS_PLUGIN_PREFIX,
  checkOutputPluginOption,
  normalizePluginOption,
  normalizePlugins,
} from './normalize-plugin-option';
import { getParallelPluginInfo } from './parallel-plugin';
import { findPropertyDescriptorInPrototypeChain } from './prototype-chain';

export async function createBundlerOptions(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  watchMode: boolean,
  closeCallbackScope?: CloseCallbackScope,
  // See internal-docs/watch-mode/implementation.md.
  configWatchHooks: boolean = watchMode,
  runBuildCallback?: BuildCallbackRunner,
  /**
   * Whether to time plugin hooks. Only `RolldownBuild` asks for it, because it is the one
   * caller that reaches `close()`, where the report is flushed. A recorder created for
   * watch, dev or scan would be written to on every hook call and never read.
   */
  measureTimings = false,
): Promise<BundlerOptionWithStopWorker> {
  // A `plugins` accessor is user code that native close can end up waiting on,
  // so run it inside the close-callback scope: a `bundle.close()` issued from
  // it is then acknowledged reentrantly. Only the accessor runs in the scope —
  // `run()` assimilates a thenable RESULT, which would swallow a user-supplied
  // thenable plugin option — and the raw value goes to `normalizePluginOption`.
  const readPluginOption = <T>(read: () => T): T => {
    if (!closeCallbackScope) return read();
    let value!: T;
    closeCallbackScope.run(() => {
      value = read();
    });
    return value;
  };

  assertParallelPluginOptionsSupported(
    readPluginOption(() => inputOptions.plugins),
    readPluginOption(() => outputOptions.plugins),
  );
  const inputPlugins = await normalizePluginOption(
    readPluginOption(() => inputOptions.plugins),
    closeCallbackScope,
  );
  const outputPlugins = await normalizePluginOption(
    readPluginOption(() => outputOptions.plugins),
    closeCallbackScope,
  );

  const logLevel = inputOptions.logLevel || LOG_LEVEL_INFO;
  const inputObjectPlugins = getObjectPlugins(inputPlugins);
  const hasUserLogCallback =
    hasDefinedProperty(inputOptions, 'onLog') ||
    hasDefinedProperty(inputOptions, 'onwarn') ||
    inputObjectPlugins.some((plugin) => hasDefinedProperty(plugin, 'onLog'));
  // Read once, like Rollup and like the plugin-less path: an accessor-backed
  // `onLog`/`onwarn` on the input options must not be re-run per log entry.
  // `snapshotPluginHooks` stays inside `invokeLogger` on purpose - plugin
  // `onLog` accessors have to execute inside the reentrancy guard.
  const inputLogHandlers = getOnLog(snapshotInputLogHandlers(inputOptions), logLevel);
  const invokeLogger: LogHandler = (level, log) =>
    getLogger(
      snapshotPluginHooks(inputObjectPlugins, 'onLog'),
      inputLogHandlers,
      logLevel,
      watchMode,
    )(level, log);
  const onLog: LogHandler =
    runBuildCallback && hasUserLogCallback
      ? (level, log) => runBuildCallback(() => invokeLogger(level, log), 'onLog')
      : invokeLogger;

  // The `outputOptions` hook is called with the input plugins and the output plugins.
  // Snapshotting makes accessor-backed hooks execute exactly once and only inside the guard.
  const outputOptionPlugins = getObjectPlugins([...inputPlugins, ...outputPlugins]);
  const callOutputOptionsHook = () =>
    PluginDriver.callOutputOptionsHook(
      snapshotPluginHooks(outputOptionPlugins, 'outputOptions'),
      outputOptions,
      onLog,
      logLevel,
      watchMode,
    );
  const invokeOutputOptionsHook = () =>
    closeCallbackScope ? closeCallbackScope.run(callOutputOptionsHook) : callOutputOptionsHook();
  const hasOutputOptionsHook = outputOptionPlugins.some((plugin) =>
    hasDefinedProperty(plugin, 'outputOptions'),
  );
  outputOptions =
    runBuildCallback && hasOutputOptionsHook
      ? runBuildCallback(callOutputOptionsHook, 'outputOptions')
      : invokeOutputOptionsHook();

  assertParallelPluginOptionsSupported(readPluginOption(() => outputOptions.plugins));
  const hookOutputPlugins = await normalizePluginOption(
    readPluginOption(() => outputOptions.plugins),
    closeCallbackScope,
  );
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

  let parallelPluginInitResult: Awaited<ReturnType<typeof initializeParallelPlugins>>;
  try {
    if (import.meta.browserBuild) {
      if (plugins.some((plugin) => getParallelPluginInfo(plugin) !== undefined)) {
        assertParallelPluginsSupported();
      }
      parallelPluginInitResult = undefined;
    } else {
      parallelPluginInitResult = await initializeParallelPlugins(plugins, watchMode);
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
      timings,
      closeCallbackScope,
      configWatchHooks,
      runBuildCallback,
    );

    // Convert `OutputOptions` to `BindingOutputOptions`
    const bindingOutputOptions = bindingifyOutputOptions(
      outputOptions,
      pluginContextData,
      timings,
      runBuildCallback,
    );

    if (
      import.meta.browserBuild &&
      runBuildCallback &&
      bindingOptionsRequireAsyncContext(
        bindingInputOptions,
        bindingOutputOptions,
        hasUserLogCallback,
      )
    ) {
      runBuildCallback(() => {}, 'browser async-context preflight');
    }

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
      releaseOptionBoxes: () => pluginContextData.releaseRetainedOptionBoxes(),
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
  /**
   * Releases the native option boxes this build's hooks retained; see
   * {@linkcode PluginContextData.releaseRetainedOptionBoxes}. Idempotent and
   * a no-op outside the threadless-WASI flavor; every consumer must call it
   * once its build reaches a terminal state (settled, scanned, or closed).
   */
  releaseOptionBoxes: () => void;
}

type SnapshotPluginHookName = 'onLog' | 'outputOptions';

function snapshotPluginHooks(plugins: Plugin[], hookName: SnapshotPluginHookName): Plugin[] {
  return plugins.map((plugin) => {
    const hook = readPropertyOnce(plugin, hookName);
    return Object.create(plugin, {
      [hookName]: {
        configurable: true,
        enumerable: findPropertyDescriptor(plugin, hookName)?.enumerable ?? true,
        value: hook,
        writable: true,
      },
    }) as Plugin;
  });
}

function snapshotInputLogHandlers(inputOptions: InputOptions): InputOptions {
  return Object.create(inputOptions, {
    onLog: {
      configurable: true,
      enumerable: findPropertyDescriptor(inputOptions, 'onLog')?.enumerable ?? true,
      value: readPropertyOnce(inputOptions, 'onLog'),
      writable: true,
    },
    onwarn: {
      configurable: true,
      enumerable: findPropertyDescriptor(inputOptions, 'onwarn')?.enumerable ?? true,
      value: readPropertyOnce(inputOptions, 'onwarn'),
      writable: true,
    },
  }) as InputOptions;
}

function hasDefinedProperty(object: object, key: PropertyKey): boolean {
  const descriptor = findPropertyDescriptor(object, key);
  if (!descriptor) return false;
  return 'value' in descriptor ? descriptor.value != null : descriptor.get != null;
}

function readPropertyOnce<T extends object, K extends keyof T>(
  object: T,
  key: K,
): T[K] | undefined {
  const descriptor = findPropertyDescriptor(object, key);
  if (!descriptor) return undefined;
  if ('value' in descriptor) return descriptor.value;
  return descriptor.get?.call(object);
}

function findPropertyDescriptor(object: object, key: PropertyKey): PropertyDescriptor | undefined {
  return findPropertyDescriptorInPrototypeChain(object, key, 'inspecting callback options');
}

/** @internal */
export function bindingOptionsRequireAsyncContext(
  inputOptions: BindingBundlerOptions['inputOptions'],
  outputOptions: BindingBundlerOptions['outputOptions'],
  hasUserLogCallback: boolean,
): boolean {
  if (
    hasUserLogCallback ||
    typeof inputOptions.external === 'function' ||
    typeof inputOptions.treeshake?.moduleSideEffects === 'function'
  ) {
    return true;
  }

  if (inputOptions.plugins.some(bindingPluginHasCallback)) {
    return true;
  }

  if (hasOwnFunctionProperty(outputOptions)) {
    return true;
  }

  return (
    outputOptions.manualCodeSplitting?.groups?.some(
      (group) => typeof group.name === 'function' || typeof group.test === 'function',
    ) === true
  );
}

function bindingPluginHasCallback(
  plugin: BindingBundlerOptions['inputOptions']['plugins'][number],
) {
  if (!plugin) return false;
  if (hasOwnFunctionProperty(plugin)) return true;

  const options = Object.getOwnPropertyDescriptor(plugin, 'options')?.value;
  return hasOwnFunctionProperty(options);
}

function hasOwnFunctionProperty(value: unknown): boolean {
  if (value === null || typeof value !== 'object') return false;
  for (const descriptor of Object.values(Object.getOwnPropertyDescriptors(value))) {
    if ('value' in descriptor && typeof descriptor.value === 'function') return true;
  }
  return false;
}
