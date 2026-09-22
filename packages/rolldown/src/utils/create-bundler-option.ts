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
import { type CloseCallbackScope, markScopeEnteringCallback } from './close-callback-scope';
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
import { findPropertyDescriptor } from '../builtin-plugin/utils';

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

  // One snapshot per object for both consumers: an accessor-backed `plugins`
  // has to fire exactly once, or the preflight read would swallow the plugins
  // the accessor only serves once. Same shape as the `outputOptions` hook
  // result below. See internal-docs/async-runtime/implementation.md.
  //
  // The input side runs all the way through normalization before the output
  // options are read at all, which is the order every release before this
  // branch had. A supported thenable input plugin settles inside that `await`,
  // and an accessor-backed `output.plugins` may answer differently once it
  // has; reading both up front took an answer from a state the build never ran
  // in, so an output plugin materialized by that settling lost its
  // `outputOptions` hook while its `renderChunk` still ran off the later
  // hook-result read.
  //
  // `assertParallelPluginOptionsSupported` is variadic but keeps no state
  // across its arguments and returns on the first descriptor it finds, with no
  // combined message, so one call per object is the same check. Each call is
  // still synchronous and still precedes its own object's normalization, which
  // is the boundary internal-docs/async-runtime/implementation.md describes.
  const inputPluginOption = readPluginOption(() => inputOptions.plugins);
  assertParallelPluginOptionsSupported(inputPluginOption);
  const inputPlugins = await normalizePluginOption(inputPluginOption, closeCallbackScope);
  const outputPluginOption = readPluginOption(() => outputOptions.plugins);
  assertParallelPluginOptionsSupported(outputPluginOption);
  const outputPlugins = await normalizePluginOption(outputPluginOption, closeCallbackScope);

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
      ? markScopeEnteringCallback<LogHandler>((level, log) =>
          runBuildCallback(() => invokeLogger(level, log), 'onLog'),
        )
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
  /** One view list per consumer; never re-reads an already captured value. */
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
 *
 * `snapshot()` hands each consumer a view that answers the hook key from the
 * captured value and delegates everything else to the plugin, with the plugin
 * as the receiver. An `Object.create(plugin, ...)` overlay delegated too, but
 * with itself as the receiver: `plugin.name` - read unsnapshotted by
 * `PluginDriver.callOutputOptionsHook` and by `getLogger` - then threw
 * `Cannot read private member` for a class plugin whose `name` getter is
 * backed by one.
 *
 * The view is a facade over a PRIVATE, empty, extensible target rather than a
 * `Proxy` on the plugin. A target with no own keys carries no `get`, `has`,
 * `ownKeys` or `getOwnPropertyDescriptor` invariant, so nothing the plugin does
 * afterwards can force a live answer out of the facade: a hook the plugin gains
 * or replaces after this pass read it - a getter redefining itself as a frozen
 * data property, another callback freezing a sibling's `onLog` - is simply not
 * observed. That is the documented conversion-time read model: presence was
 * decided from the captured value, `hasUserLogCallback` and `hasOutputOptionsHook`
 * were decided with it, and a hook the facade did not capture would run outside
 * the reentrancy guard those flags install.
 *
 * INTERNAL ONLY. The private-target trick works here because no consumer of
 * these views is user code: `getSortedPlugins`, `getObjectPlugins` and
 * `getParallelPluginInfo` only inspect them, `PluginDriver.callOutputOptionsHook`
 * and `getLogger` call the hook with a `MinimalPluginContextImpl` (or a literal
 * log context) as `this`, never the view. A view handed to user code must keep
 * the original as the `Proxy` target so that freezing or sealing it stays
 * legal - see `createWatchOptionSnapshotView` in `api/watch/watcher.ts`.
 *
 * `builtin-plugin/utils.ts`'s `createCallbackSnapshotView` is the same shape but
 * not reusable as is: it pins delegated non-configurable descriptors onto its
 * private target, always delegates with the original as the receiver, and traps
 * no `set`/`defineProperty`/`deleteProperty`, so writes would land on the
 * private target instead of the plugin.
 */
function capturePluginHooks(
  plugins: Plugin[],
  hookName: SnapshotPluginHookName,
): CapturedPluginHooks {
  let present = false;
  const captured = plugins.map((plugin): CapturedPluginHook => {
    const descriptor = findPropertyDescriptor(plugin, hookName);
    // oxlint-disable-next-line typescript/unbound-method -- only tested for shape, never invoked
    if (descriptor && !('value' in descriptor) && typeof descriptor.get === 'function') {
      // An accessor with a getter: calling that getter is user code, so the
      // read belongs inside the boundary and the entry counts as present on its
      // own - whatever the getter returns, running it is what must be guarded.
      present = true;
      return { plugin, deferred: true, value: undefined };
    }
    // Every other shape - a data property, a getter-less accessor, or a key the
    // walk found no descriptor for - is one plain read. A getter-less accessor
    // has nothing to call, and on a `Proxy` the `get` trap is the only way to
    // observe the hook at all.
    const value = Reflect.get(plugin, hookName, plugin);
    if (hookWouldRun(value)) present = true;
    return { plugin, deferred: false, value };
  });

  return {
    present,
    snapshot: () =>
      captured.map(({ plugin, deferred, value }) => {
        // The deferred read happens here, inside `snapshot()`, which every
        // caller invokes inside the callback boundary.
        const hookValue = deferred ? readPropertyOnce(plugin, hookName) : value;
        // A fresh, extensible object with no own keys. Because the facade never
        // reports one of ITS keys as non-configurable, and never reports the
        // target as non-extensible, every trap below is free to answer from the
        // plugin or from the captured value without meeting an invariant.
        const target: Record<PropertyKey, never> = {};
        const view: Plugin = new Proxy(target, {
          get(_target, key, receiver) {
            // The captured value wins, whatever the plugin looks like now.
            if (key === hookName) return hookValue;
            // The plugin is the receiver, so `plugin.name` - which
            // `callOutputOptionsHook` and `getLogger` read straight off this
            // view - reaches a getter backed by a private field. An object
            // derived from the view keeps its own receiver.
            return Reflect.get(plugin, key, receiver === view ? plugin : receiver);
          },
          has(_target, key) {
            // Same answer the `get` trap gives, so `in` cannot disagree with a
            // read. No consumer reaches a hook this way, but a facade whose
            // `has` contradicted its `get` would be a trap for the next one.
            if (key === hookName) return hookValue !== undefined;
            return Reflect.has(plugin, key);
          },
          getPrototypeOf() {
            // `getObjectPlugins` tests `plugin instanceof BuiltinPlugin`.
            return Reflect.getPrototypeOf(plugin);
          },
          ownKeys() {
            const keys = Reflect.ownKeys(plugin);
            // Only add the hook key when the facade answers for it and the
            // plugin does not already list it - `ownKeys` rejects duplicates.
            if (hookValue !== undefined && !keys.includes(hookName)) {
              return [...keys, hookName];
            }
            return keys;
          },
          getOwnPropertyDescriptor(_target, key) {
            if (key === hookName) {
              return hookValue === undefined
                ? undefined
                : { configurable: true, enumerable: true, value: hookValue, writable: true };
            }
            const descriptor = Reflect.getOwnPropertyDescriptor(plugin, key);
            // A facade may not report a non-configurable key its target lacks,
            // and the target deliberately has none, so report the plugin's
            // descriptor as configurable. Only `getParallelPluginInfo` reads
            // these, and it looks at `value` alone.
            if (descriptor) descriptor.configurable = true;
            return descriptor;
          },
          defineProperty(_target, key, descriptor) {
            return Reflect.defineProperty(plugin, key, descriptor);
          },
          deleteProperty(_target, key) {
            return Reflect.deleteProperty(plugin, key);
          },
          set(_target, key, newValue, receiver) {
            return Reflect.set(plugin, key, newValue, receiver === view ? plugin : receiver);
          },
        }) as unknown as Plugin;
        return view;
      }),
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

/**
 * Overlay of the input options whose `onLog` and `onwarn` answer the one read
 * this pass took. `Object.create` is enough here, unlike the plugin hook view
 * above: every consumer - the two {@linkcode hookWouldRun} presence tests and
 * {@linkcode getOnLog} - reads `onLog` and `onwarn` and nothing else, so no
 * read is ever delegated to the original with the overlay as its receiver.
 */
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
