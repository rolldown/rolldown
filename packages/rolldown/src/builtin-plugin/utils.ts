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

export function makeBuiltinPluginCallable(
  plugin: BuiltinPlugin,
): BuiltinPlugin & BindingCallableBuiltinPluginLike {
  let callablePlugin = new BindingCallableBuiltinPlugin(bindingifyBuiltInPlugin(plugin));

  const wrappedPlugin: Partial<BindingCallableBuiltinPluginLike> & BuiltinPlugin = plugin;
  for (const key in callablePlugin) {
    // @ts-expect-error
    wrappedPlugin[key] = async function (...args) {
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
  }
  return wrappedPlugin as BuiltinPlugin & BindingCallableBuiltinPluginLike;
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
