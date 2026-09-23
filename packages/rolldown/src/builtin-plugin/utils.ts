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
};

export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  const context: CallableBuiltinPluginContext = {
    plugin,
    native: new BindingCallableBuiltinPlugin(bindingifyBuiltInPlugin(plugin)),
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
