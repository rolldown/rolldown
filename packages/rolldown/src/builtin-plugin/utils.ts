import {
  type BindingBuiltinPlugin,
  type BindingBuiltinPluginName,
  BindingCallableBuiltinPlugin,
  type BindingViteManifestPluginConfig,
} from '../binding.cjs';
import { error, logPluginError } from '../log/logs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import type { ViteManifestPluginConfig } from './vite-manifest-plugin';

type BindingCallableBuiltinPluginLike = {
  [K in keyof BindingCallableBuiltinPlugin]: BindingCallableBuiltinPlugin[K];
};

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

type CallableBuiltinPluginContext = {
  plugin: BuiltinPlugin;
  native: BindingCallableBuiltinPlugin;
  callbacks: Function[];
};

export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  const callbacks: Function[] = [];
  const binding = bindingifyBuiltInPlugin(plugin);
  const options = binding.options;
  if (options !== null && (typeof options === 'object' || typeof options === 'function')) {
    binding.options = new Proxy(
      {},
      {
        get(_target, key) {
          const value = Reflect.get(options, key);
          if (typeof value !== 'function') return value;
          callbacks.push(value);
          return invokeWeakCallback.bind(
            undefined,
            new WeakRef(value),
            `${plugin.name}.${String(key)}`,
          );
        },
      },
    );
  }
  const context: CallableBuiltinPluginContext = {
    plugin,
    native: new BindingCallableBuiltinPlugin(binding),
    callbacks,
  };

  const wrappedPlugin: Partial<BindingCallableBuiltinPluginLike> & BuiltinPlugin = plugin;
  for (const key in context.native) {
    const wrappedHook = invokeBuiltinHook.bind(context, key);
    const order = context.native.getOrder(key);
    if (order == undefined) {
      // @ts-expect-error
      wrappedPlugin[key] = wrappedHook;
    } else {
      // @ts-expect-error
      wrappedPlugin[key] = { handler: wrappedHook, order };
    }
  }
  return wrappedPlugin as BuiltinPlugin & BindingCallableBuiltinPluginLike;
}

async function invokeBuiltinHook(this: CallableBuiltinPluginContext, key: string, ...args: any[]) {
  try {
    // @ts-expect-error
    return await this.native[key](...args);
  } catch (e: any) {
    if (e instanceof Error && !e.stack?.includes('at ')) {
      Error.captureStackTrace(
        e,
        // @ts-expect-error
        this.plugin[key],
      );
    }
    return error(
      logPluginError(e, this.plugin.name, {
        hook: key,
        id: key === 'transform' ? args[2] : undefined,
      }),
    );
  }
}

function invokeWeakCallback(reference: WeakRef<Function>, name: string, ...args: unknown[]) {
  const callback = reference.deref();
  if (!callback) {
    throw new Error(`The callback for ${name} was released`);
  }
  return Reflect.apply(callback, undefined, args);
}

export function bindingifyBuiltInPlugin(plugin: BuiltinPlugin): BindingBuiltinPlugin {
  return {
    __name: plugin.name,
    options: plugin._options,
  };
}

export function bindingifyManifestPlugin(
  plugin: BuiltinPlugin,
  pluginContextData: PluginContextData,
): BindingBuiltinPlugin {
  const { isOutputOptionsForLegacyChunks, ...options } =
    plugin._options as ViteManifestPluginConfig;
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
