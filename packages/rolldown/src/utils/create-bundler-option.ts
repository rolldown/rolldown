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
 * `snapshot()` hands each consumer a view of the plugin that answers the hook
 * key from the captured value and delegates everything else to the plugin
 * itself, as the target AND as the receiver. An `Object.create(plugin, ...)`
 * overlay delegated too, but with itself as the receiver: `plugin.name` - read
 * unsnapshotted by `PluginDriver.callOutputOptionsHook` and by `getLogger` -
 * then threw `Cannot read private member` for a class plugin whose `name`
 * getter is backed by one. Enumerability comes from the plugin for the same
 * reason.
 *
 * The one thing the view cannot answer freely is a hook the plugin has since
 * pinned down: where the plugin's own descriptor is non-configurable, the
 * `Proxy` `get` invariant fixes the answer, and the view gives that answer
 * instead of the captured value - otherwise the read throws `TypeError`
 * before any hook runs.
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
        const view: Plugin = new Proxy(plugin, {
          get(target, key, receiver) {
            if (key === hookName) {
              // A hook the plugin replaced after this pass read it - a getter
              // redefining itself as a frozen data property, another callback
              // overwriting a sibling plugin's hook and freezing it - leaves an
              // own descriptor the `get` invariant fixes the answer for. Answer
              // with it: reporting the captured value there is a `TypeError`
              // thrown before any hook runs. The check only inspects the
              // descriptor; it never calls anything.
              const descriptor = Reflect.getOwnPropertyDescriptor(target, hookName);
              if (descriptor && !descriptor.configurable) {
                // A non-configurable non-writable data property admits exactly
                // its own value.
                if ('value' in descriptor) {
                  if (!descriptor.writable) return descriptor.value;
                  // oxlint-disable-next-line typescript/unbound-method -- shape test only, never invoked
                } else if (descriptor.get === undefined) {
                  // A non-configurable accessor with nothing to call admits
                  // exactly `undefined`.
                  return undefined;
                }
              }
              return hookValue;
            }
            // The plugin is the receiver, so `plugin.name` - which
            // `callOutputOptionsHook` and `getLogger` read straight off this
            // view - reaches a getter backed by a private field. An object
            // derived from the view keeps its own receiver.
            return Reflect.get(target, key, receiver === view ? target : receiver);
          },
          // No `has` trap: every consumer of these views - `getSortedPlugins`,
          // `PluginDriver.callOutputOptionsHook` and `getLogger` - reaches the
          // hook by reading it, never by `in`, `Reflect.has`, `Object.keys` or
          // `for...in`. Reporting a key the plugin does not own would violate
          // the `has` invariant on a frozen plugin for no consumer's benefit,
          // so the default delegation answers.
        });
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
