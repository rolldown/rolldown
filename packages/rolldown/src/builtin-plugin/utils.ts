import {
  type BindingBuiltinPlugin,
  type BindingBuiltinPluginName,
  BindingCallableBuiltinPlugin,
  type BindingViteDynamicImportVarsPluginConfig,
  type BindingViteManifestPluginConfig,
  type BindingViteReporterPluginConfig,
  type BindingViteResolvePluginConfig,
} from '../binding.cjs';
import { error, logPluginError } from '../log/logs';
import type { BuildCallbackRunner } from '../plugin/bindingify-plugin';
import type { PluginContextData } from '../plugin/plugin-context-data';
import type { TypeAssert } from '../types/assert';
import { findPropertyDescriptorInPrototypeChain } from '../utils/prototype-chain';
import type { ViteManifestPluginConfig } from './vite-manifest-plugin';

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K];
};

type CallbackPropertyName<T> = {
  [K in keyof T]-?: NonNullable<T[K]> extends (...args: any[]) => any ? K : never;
}[keyof T];

const DYNAMIC_IMPORT_VARS_CALLBACKS = [
  'resolver',
] as const satisfies readonly CallbackPropertyName<BindingViteDynamicImportVarsPluginConfig>[];
const MANIFEST_CALLBACKS = [
  'isOutputOptionsForLegacyChunks',
  'cssEntries',
] as const satisfies readonly CallbackPropertyName<ViteManifestPluginConfig>[];
const REPORTER_CALLBACKS = [
  'logInfo',
] as const satisfies readonly CallbackPropertyName<BindingViteReporterPluginConfig>[];
const RESOLVE_CALLBACKS = [
  'finalizeBareSpecifier',
  'finalizeOtherSpecifiers',
  'resolveSubpathImports',
  'onWarn',
  'onDebug',
] as const satisfies readonly CallbackPropertyName<BindingViteResolvePluginConfig>[];

// eslint-disable @typescript-eslint/no-unsafe-declaration-merging
export class BuiltinPlugin {
  /** Vite-specific option to control plugin ordering */
  enforce?: 'pre' | 'post';

  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {}
}

export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  let callablePlugin = new BindingCallableBuiltinPlugin(bindingifyBuiltInPlugin(plugin));

  const wrappedPlugin: Partial<BindingCallableBuiltinPluginLike> & BuiltinPlugin = plugin;
  for (const key in callablePlugin) {
    const wrappedHook = async function (...args: any[]) {
      try {
        // @ts-expect-error
        return await callablePlugin[key](...args);
      } catch (e: any) {
        if (e instanceof Error && !e.stack?.includes('at ')) {
          Error.captureStackTrace(
            e,
            // @ts-expect-error
            wrappedPlugin[key],
          );
        }
        return error(
          logPluginError(e, plugin.name, {
            hook: key,
            id: key === 'transform' ? args[2] : undefined,
          }),
        );
      }
    };

    const order = callablePlugin.getOrder(key);
    if (order == undefined) {
      // @ts-expect-error
      wrappedPlugin[key] = wrappedHook;
    } else {
      // @ts-expect-error
      wrappedPlugin[key] = {
        handler: wrappedHook,
        order,
      };
    }
  }
  return wrappedPlugin as BuiltinPlugin & BindingCallableBuiltinPluginLike;
}

export function bindingifyBuiltInPlugin(
  plugin: BuiltinPlugin,
  runBuildCallback?: BuildCallbackRunner,
): BindingBuiltinPlugin {
  let options = plugin._options;
  switch (plugin.name) {
    case 'builtin:vite-dynamic-import-vars':
      options = wrapCallbackProperties(
        options as BindingViteDynamicImportVarsPluginConfig | undefined,
        DYNAMIC_IMPORT_VARS_CALLBACKS,
        runBuildCallback,
      );
      break;
    case 'builtin:vite-reporter':
      options = wrapCallbackProperties(
        options as BindingViteReporterPluginConfig,
        REPORTER_CALLBACKS,
        runBuildCallback,
      );
      break;
    case 'builtin:vite-resolve':
      options = wrapCallbackProperties(
        options as BindingViteResolvePluginConfig,
        RESOLVE_CALLBACKS,
        runBuildCallback,
      );
      break;
  }
  return {
    __name: plugin.name,
    options,
  };
}

export function bindingifyManifestPlugin(
  plugin: BuiltinPlugin,
  pluginContextData: PluginContextData,
  runBuildCallback?: BuildCallbackRunner,
): BindingBuiltinPlugin {
  const wrappedOptions = wrapCallbackProperties(
    plugin._options as ViteManifestPluginConfig,
    MANIFEST_CALLBACKS,
    runBuildCallback,
  );
  const { isOutputOptionsForLegacyChunks, ...options } = wrappedOptions;
  return {
    __name: plugin.name,
    options: {
      ...options,
      isLegacy: isOutputOptionsForLegacyChunks
        ? (opts) => {
            return isOutputOptionsForLegacyChunks(pluginContextData.getOutputOptions(opts));
          }
        : undefined,
    } as BindingViteManifestPluginConfig,
  };
}

function wrapCallbackProperties<T extends object>(
  options: T,
  keys: readonly CallbackPropertyName<T>[],
  runBuildCallback?: BuildCallbackRunner,
): T;
function wrapCallbackProperties<T extends object>(
  options: T | undefined,
  keys: readonly CallbackPropertyName<T>[],
  runBuildCallback?: BuildCallbackRunner,
): T | undefined;
function wrapCallbackProperties<T extends object>(
  options: T | undefined,
  keys: readonly CallbackPropertyName<T>[],
  runBuildCallback?: BuildCallbackRunner,
): T | undefined {
  if (!options) return options;

  // Every callback key is read exactly once, here. A key the bounded walk
  // found no descriptor for may be served by a `Proxy` `get` trap, and a trap
  // is free to answer differently on every read, so this first value is the
  // only one the binding may ever see.
  const snapshot = new Map<PropertyKey, PropertyDescriptor>();

  for (const key of keys) {
    const descriptor = findPropertyDescriptor(options, key);
    const callback = readPropertyOnce(options, key, descriptor, runBuildCallback);
    const snapshotted: PropertyDescriptor = {
      configurable: true,
      enumerable: descriptor?.enumerable ?? true,
      value:
        typeof callback === 'function'
          ? (...args: unknown[]) => {
              const invoke = () => Reflect.apply(callback, options, args);
              return runBuildCallback ? runBuildCallback(invoke, String(key)) : invoke();
            }
          : callback,
      writable: true,
    };
    snapshot.set(key, snapshotted);
  }

  // The pass performs exactly one `[[Get]]` per callback key, so it must always
  // pin what it read. Handing the original object over instead lets N-API's
  // `napi_get_named_property` read the same key a second time, where a stateful
  // `get` trap is free to answer with a raw callback. A callback-free config
  // still asks for no async context provider, because building the overlay runs
  // no user code and enters no runner.
  //
  // The overlay is what the binding gets, whatever found the keys: rebuilding a
  // plain object from the original's own descriptors would drop the fields only
  // a `get` trap can answer.
  return createCallbackSnapshotView(options, snapshot);
}

/**
 * Binding-facing view of a callback-bearing built-in config. Snapshotted
 * callback keys are answered from a private target, so N-API cannot re-read
 * them and a stateful `get` trap cannot hand the native side a raw callback.
 * Everything else is delegated to the original object with the original
 * receiver, so required fields a `get` trap serves - and that no descriptor
 * walk can see - still reach the binding.
 */
function createCallbackSnapshotView<T extends object>(
  options: T,
  snapshot: Map<PropertyKey, PropertyDescriptor>,
): T {
  // Owning the snapshotted keys on a private target keeps the `Proxy`
  // invariants satisfiable even when the original config is frozen.
  const target: Record<PropertyKey, unknown> = {};
  for (const [key, descriptor] of snapshot) {
    Object.defineProperty(target, key, descriptor);
  }

  return new Proxy(target, {
    get(target, key, receiver) {
      if (snapshot.has(key)) return Reflect.get(target, key, receiver);
      return Reflect.get(options, key, options);
    },
    getOwnPropertyDescriptor(target, key) {
      if (snapshot.has(key)) return Reflect.getOwnPropertyDescriptor(target, key);
      const descriptor = Reflect.getOwnPropertyDescriptor(options, key);
      // A `Proxy` may not report a non-configurable property its target does
      // not have, so mirror it before answering with it.
      if (descriptor && !descriptor.configurable) {
        Object.defineProperty(target, key, descriptor);
      }
      return descriptor;
    },
    getPrototypeOf() {
      return Reflect.getPrototypeOf(options);
    },
    has(target, key) {
      return snapshot.has(key) || Reflect.has(options, key);
    },
    ownKeys(target) {
      const keys = new Set<string | symbol>(Reflect.ownKeys(target));
      for (const key of Reflect.ownKeys(options)) {
        keys.add(key);
      }
      return [...keys];
    },
  }) as unknown as T;
}

function readPropertyOnce<T extends object, K extends keyof T>(
  object: T,
  key: K,
  descriptor: PropertyDescriptor | undefined,
  runBuildCallback?: BuildCallbackRunner,
): T[K] | undefined {
  // The bounded walk that produced `descriptor` is what bounds a cyclic or
  // fabricated prototype chain, so it has already run before this read. The
  // value itself always comes from this one `[[Get]]` with the original
  // receiver - the very operation N-API's `napi_get_named_property` performs -
  // so the binding can never see a value that differs from what any other JS
  // reader gets. The descriptor only classifies the read.
  const read = () => Reflect.get(object, key, object) as T[K] | undefined;
  // oxlint-disable-next-line typescript/unbound-method -- only tested for shape, never invoked
  if (descriptor && !('value' in descriptor) && typeof descriptor.get === 'function') {
    // An accessor with a getter: calling that getter is user code, so the read
    // belongs inside the boundary.
    return runBuildCallback ? runBuildCallback(read, String(key)) : read();
  }
  // Every other shape - a data property, a getter-less accessor, or a key the
  // walk found no descriptor for - is a plain read: nothing gets called, so it
  // stays outside the boundary and a callback-free config asks for no async
  // context provider. A getter-less accessor is read like the rest rather than
  // skipped: skipping it would pin `undefined` without spending the one
  // `[[Get]]` this pass owes the key, and would drop a callback a `Proxy` `get`
  // trap answers for while N-API's own `[[Get]]` would still have found it.
  return read();
}

export function findPropertyDescriptor(
  object: object,
  key: PropertyKey,
): PropertyDescriptor | undefined {
  return findPropertyDescriptorInPrototypeChain(object, key, 'inspecting callback options');
}

function _assertCallbackInventories() {
  type _ = TypeAssert<
    [
      Exclude<
        CallbackPropertyName<BindingViteDynamicImportVarsPluginConfig>,
        (typeof DYNAMIC_IMPORT_VARS_CALLBACKS)[number]
      >,
      Exclude<CallbackPropertyName<ViteManifestPluginConfig>, (typeof MANIFEST_CALLBACKS)[number]>,
      Exclude<
        CallbackPropertyName<BindingViteReporterPluginConfig>,
        (typeof REPORTER_CALLBACKS)[number]
      >,
      Exclude<
        CallbackPropertyName<BindingViteResolvePluginConfig>,
        (typeof RESOLVE_CALLBACKS)[number]
      >,
    ] extends [never, never, never, never]
      ? true
      : false
  >;
}
