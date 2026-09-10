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
import { runWithRuntimeLease } from '../utils/run-with-runtime-lease';
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
        return await runWithRuntimeLease(
          // @ts-expect-error
          () => callablePlugin[key](...args),
          `Callable builtin ${key} hook and runtime release both failed`,
        );
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

  const callbackDescriptors = new Map<PropertyKey, PropertyDescriptor>();
  for (const key of keys) {
    const descriptor = findPropertyDescriptor(options, key);
    if (!descriptor) continue;
    const callback = readPropertyOnce(options, key, runBuildCallback);
    const isAccessor = !('value' in descriptor);
    if (typeof callback !== 'function' && !isAccessor) continue;

    callbackDescriptors.set(key, {
      configurable: true,
      enumerable: descriptor.enumerable ?? true,
      value:
        typeof callback === 'function'
          ? (...args: unknown[]) => {
              const invoke = () => Reflect.apply(callback, options, args);
              return runBuildCallback ? runBuildCallback(invoke, String(key)) : invoke();
            }
          : callback,
      writable: true,
    });
  }
  if (callbackDescriptors.size === 0) return options;

  const descriptors = Object.getOwnPropertyDescriptors(options);
  for (const [key, descriptor] of callbackDescriptors) {
    Reflect.set(descriptors, key, descriptor);
  }
  return Object.create(Object.getPrototypeOf(options), descriptors) as T;
}

function readPropertyOnce<T extends object, K extends keyof T>(
  object: T,
  key: K,
  runBuildCallback?: BuildCallbackRunner,
): T[K] | undefined {
  const descriptor = findPropertyDescriptor(object, key);
  if (!descriptor) return undefined;
  if ('value' in descriptor) return descriptor.value;
  // oxlint-disable-next-line typescript/unbound-method -- invoked with its receiver below
  const getter = descriptor.get;
  if (!getter) return undefined;
  const read = () => Reflect.apply(getter, object, []);
  return runBuildCallback ? runBuildCallback(read, String(key)) : read();
}

function findPropertyDescriptor(object: object, key: PropertyKey): PropertyDescriptor | undefined {
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
