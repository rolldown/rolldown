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
  // Capture the plugin `onLog` hooks once, up front: presence is decided by the
  // captured VALUE, with the same test the log runner applies before calling
  // it, so a plugin whose handler only a `get` trap can answer for is seen here
  // and reaches the reentrancy guard like any other.
  const pluginLogHooks = capturePluginHooks(inputObjectPlugins, 'onLog');
  // Read once, like Rollup and like the plugin-less path: an accessor-backed
  // `onLog`/`onwarn` on the input options must not be re-run per log entry.
  // The plugin snapshot stays inside `invokeLogger` on purpose - plugin
  // `onLog` accessors have to execute inside the reentrancy guard.
  const snapshottedInputOptions = snapshotInputLogHandlers(inputOptions);
  const hasUserLogCallback =
    hookWouldRun(snapshottedInputOptions.onLog) ||
    hookWouldRun(snapshottedInputOptions.onwarn) ||
    pluginLogHooks.present;
  const inputLogHandlers = getOnLog(snapshottedInputOptions, logLevel);
  const invokeLogger: LogHandler = (level, log) =>
    getLogger(pluginLogHooks.snapshot(), inputLogHandlers, logLevel, watchMode)(level, log);
  const onLog: LogHandler =
    runBuildCallback && hasUserLogCallback
      ? (level, log) => runBuildCallback(() => invokeLogger(level, log), 'onLog')
      : invokeLogger;

  // The `outputOptions` hook is called with the input plugins and the output plugins.
  // Snapshotting makes accessor-backed hooks execute exactly once and only inside the guard.
  const outputOptionPlugins = getObjectPlugins([...inputPlugins, ...outputPlugins]);
  const outputOptionsHooks = capturePluginHooks(outputOptionPlugins, 'outputOptions');
  const callOutputOptionsHook = () =>
    PluginDriver.callOutputOptionsHook(
      outputOptionsHooks.snapshot(),
      outputOptions,
      onLog,
      logLevel,
      watchMode,
    );
  const invokeOutputOptionsHook = () =>
    closeCallbackScope ? closeCallbackScope.run(callOutputOptionsHook) : callOutputOptionsHook();
  const hasOutputOptionsHook = outputOptionsHooks.present;
  outputOptions =
    runBuildCallback && hasOutputOptionsHook
      ? runBuildCallback(callOutputOptionsHook, 'outputOptions')
      : invokeOutputOptionsHook();

  // One snapshot for both consumers: an accessor-backed `plugins` on the hook
  // result has to fire exactly once, or the preflight read would swallow the
  // plugins the hook injected. See internal-docs/async-runtime/implementation.md.
  const hookOutputPluginOption = readPluginOption(() => outputOptions.plugins);
  assertParallelPluginOptionsSupported(hookOutputPluginOption);
  const hookOutputPlugins = await normalizePluginOption(hookOutputPluginOption, closeCallbackScope);
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
    measureTimings &&
    (inputOptions.checks?.bundlerTimings ?? inputOptions.checks?.pluginTimings ?? true)
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
      onLog,
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
   * Releases the native option boxes and build-scoped plugin-context boxes
   * this build's hooks retained; see
   * {@linkcode PluginContextData.releaseRetainedOptionBoxes}. Idempotent and
   * a no-op outside the threadless-WASI flavor; every consumer must call it
   * once its build reaches a terminal state (settled, scanned, or closed).
   */
  releaseOptionBoxes: () => void;
}

type SnapshotPluginHookName = 'onLog' | 'outputOptions';

interface CapturedPluginHook {
  plugin: Plugin;
  enumerable: boolean;
  /**
   * Set when the bounded walk found an accessor that has a getter: calling that
   * getter is user code, so the read is deferred to `snapshot()`, which the
   * callers invoke inside the callback boundary.
   */
  deferred: boolean;
  /** What the capture read. Only meaningful when `deferred` is false. */
  value: unknown;
}

interface CapturedPluginHooks {
  /**
   * Whether any plugin supplies the hook. A value the capture read is judged
   * with {@linkcode hookWouldRun}, the test its consumer applies; a deferred
   * getter always counts, because calling the getter is itself user code that
   * has to happen inside the boundary.
   */
  present: boolean;
  /** One overlay list per consumer; never re-reads an already captured value. */
  snapshot: () => Plugin[];
}

/**
 * Reads a hook off every plugin exactly once and decides from the captured
 * VALUE - never from the descriptor alone - whether that plugin supplies it. A
 * `Proxy` serving the hook from its `get` trap answers no descriptor walk, so a
 * descriptor-only test reports "no hook" for a hook that is then found and run,
 * and the run happens outside the callback boundary. The descriptor decides only
 * whether the read calls user code, the same rule `builtin-plugin/utils.ts`
 * applies to built-in option callbacks. See
 * internal-docs/async-context/implementation.md.
 */
function capturePluginHooks(
  plugins: Plugin[],
  hookName: SnapshotPluginHookName,
): CapturedPluginHooks {
  let present = false;
  const captured = plugins.map((plugin): CapturedPluginHook => {
    const descriptor = findPropertyDescriptor(plugin, hookName);
    const enumerable = descriptor?.enumerable ?? true;
    // oxlint-disable-next-line typescript/unbound-method -- only tested for shape, never invoked
    if (descriptor && !('value' in descriptor) && typeof descriptor.get === 'function') {
      // An accessor with a getter: calling that getter is user code, so the
      // read belongs inside the boundary and the entry counts as present on its
      // own - whatever the getter returns, running it is what must be guarded.
      present = true;
      return { plugin, enumerable, deferred: true, value: undefined };
    }
    // Every other shape - a data property, a getter-less accessor, or a key the
    // walk found no descriptor for - is one plain read. A getter-less accessor
    // has nothing to call, and on a `Proxy` the `get` trap is the only way to
    // observe the hook at all.
    const value = Reflect.get(plugin, hookName, plugin);
    if (hookWouldRun(value)) present = true;
    return { plugin, enumerable, deferred: false, value };
  });

  return {
    present,
    snapshot: () =>
      captured.map(
        ({ plugin, enumerable, deferred, value }) =>
          Object.create(plugin, {
            [hookName]: {
              configurable: true,
              enumerable,
              value: deferred ? readPropertyOnce(plugin, hookName) : value,
              writable: true,
            },
          }) as Plugin,
      ),
  };
}

/**
 * The test every consumer of these values applies before calling one:
 * `getSortedPlugins`, `PluginDriver.callOutputOptionsHook` and `getLogger` all
 * gate a plugin hook on its truthiness, and `getOnLog` gates the input
 * `onLog`/`onwarn` handlers the same way. Presence has to ask exactly this, or
 * the build guards a value nobody runs - a falsy answer from a `get` trap makes
 * a callback-free browser build demand an async context provider - or runs a
 * value it never guarded.
 */
function hookWouldRun(value: unknown): boolean {
  return Boolean(value);
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

function readPropertyOnce<T extends object, K extends keyof T>(
  object: T,
  key: K,
): T[K] | undefined {
  // Walk first and discard the result: the walk is what bounds a cyclic or
  // fabricated prototype chain, and it has to throw here, before the read.
  // The read itself must go through `Reflect.get` so that a `Proxy` serving
  // or decorating the hook from its `get` trap is observed rather than
  // masked by the `undefined` this would otherwise snapshot. Still exactly
  // one read, so accessor-backed hooks keep firing once, inside the guard.
  findPropertyDescriptor(object, key);
  return Reflect.get(object, key, object) as T[K] | undefined;
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
