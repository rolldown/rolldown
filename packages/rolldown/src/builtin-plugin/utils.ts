import {
  type BindingBuiltinPlugin,
  type BindingBuiltinPluginName,
  BindingCallableBuiltinPlugin,
  type BindingOutputChunk,
  type BindingOutputs,
  type BindingViteCssPostPluginConfig,
  type BindingViteHtmlPluginConfig,
  type BindingViteManifestPluginConfig,
} from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import type { LogLevelOption } from '../log/logging';
import { error, logPluginError } from '../log/logs';
import {
  type MinimalPluginContext,
  MinimalPluginContextImpl,
} from '../plugin/minimal-plugin-context';
import type { PluginContextData } from '../plugin/plugin-context-data';
import {
  transformToOutputBundle,
  transformToRollupOutputChunk,
} from '../utils/transform-to-rollup-output';
import type { ViteCssPostPluginConfig } from './vite-css-post-plugin';
import type { IndexHtmlTransformContext, ViteHtmlPluginOptions } from './vite-html-plugin';
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

export function bindingifyCSSPostPlugin(
  plugin: BuiltinPlugin,
  pluginContextData: PluginContextData,
): BindingBuiltinPlugin {
  const { isOutputOptionsForLegacyChunks, ...options } = plugin._options as ViteCssPostPluginConfig;
  return {
    __name: plugin.name,
    options: {
      ...options,
      isLegacy: isOutputOptionsForLegacyChunks
        ? (opts) => {
            return isOutputOptionsForLegacyChunks(pluginContextData.getOutputOptions(opts));
          }
        : undefined,
      cssScopeTo() {
        const cssScopeTo: Record<string, readonly [string, string | undefined]> = {};
        for (const [id, opts] of pluginContextData.moduleOptionMap.entries()) {
          if (opts?.meta.vite?.cssScopeTo) {
            cssScopeTo[id] = opts.meta.vite.cssScopeTo;
          }
        }
        return cssScopeTo;
      },
    } as BindingViteCssPostPluginConfig,
  };
}

export function bindingifyViteHtmlPlugin(
  plugin: BuiltinPlugin,
  onLog: LogHandler,
  logLevel: LogLevelOption,
  watchMode: boolean,
  pluginContextData: PluginContextData,
): BindingBuiltinPlugin {
  const { preHooks, normalHooks, postHooks, applyHtmlTransforms, ...options } =
    plugin._options as ViteHtmlPluginOptions;
  if (preHooks.length + normalHooks.length + postHooks.length > 0) {
    return {
      __name: plugin.name,
      options: {
        ...options,
        transformIndexHtml: async (
          html: string,
          path: string,
          filename: string,
          hook: 'transform' | 'generateBundle',
          output?: BindingOutputs,
          chunk?: BindingOutputChunk,
        ): Promise<string> => {
          const pluginContext = new MinimalPluginContextImpl(
            onLog,
            logLevel,
            plugin.name,
            watchMode,
            'transformIndexHtml',
          ) as MinimalPluginContext;

          const context: IndexHtmlTransformContext = {
            path,
            filename,
            bundle: output
              ? transformToOutputBundle(pluginContext, output, {
                  updated: new Set(),
                  deleted: new Set(),
                })
              : undefined,
            chunk: chunk ? transformToRollupOutputChunk(chunk) : undefined,
          };

          switch (hook) {
            case 'transform':
              return await applyHtmlTransforms(html, preHooks, pluginContext, context);
            case 'generateBundle':
              return await applyHtmlTransforms(
                html,
                [...normalHooks, ...postHooks],
                pluginContext,
                context,
              );
          }
        },
        setModuleSideEffects(id: string) {
          let opts = pluginContextData.getModuleOption(id);
          pluginContextData.updateModuleOption(id, {
            moduleSideEffects: true,
            meta: opts.meta,
            invalidate: true,
          });
        },
      } as BindingViteHtmlPluginConfig,
    };
  }
  return {
    __name: plugin.name,
    options: plugin._options,
  };
}
