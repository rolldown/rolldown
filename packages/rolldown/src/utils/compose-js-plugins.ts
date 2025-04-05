import * as R from 'remeda';
import { TupleToUnion } from 'type-fest';
import { BuiltinPlugin } from '../builtin-plugin/constructors';
import { PluginHookNames } from '../constants/plugin';
import { SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF } from '../constants/plugin-context';
import {
  ModuleSideEffects,
  Plugin,
  PrivateResolveIdExtraOptions,
  RolldownPlugin,
} from '../plugin';
import {
  PluginContext,
  PrivatePluginContextResolveOptions,
} from '../plugin/plugin-context';
import { TransformPluginContext } from '../plugin/transform-plugin-context';
import { AssertNever } from '../types/assert';
import { isNullish } from './misc';
import { normalizeHook } from './normalize-hook';
import { isPluginHookName } from './plugin';

const unsupportedHookName = [
  'augmentChunkHash',
  'generateBundle',
  'moduleParsed',
  'onLog',
  'options',
  'outputOptions',
  'renderError',
  'renderStart',
  'resolveDynamicImport',
  'writeBundle',
] as const;
const unsupportedHooks: Set<string> = new Set(unsupportedHookName);

type UnsupportedHookNames = TupleToUnion<typeof unsupportedHookName>;
type SupportedHookNames = Exclude<PluginHookNames, UnsupportedHookNames>;

function isUnsupportedHooks(
  hookName: string,
): hookName is UnsupportedHookNames {
  return unsupportedHooks.has(hookName);
}

function createComposedPlugin(plugins: Plugin[]): Plugin {
  // Throw errors if we try to merge plugins with unsupported hooks

  const names: string[] = [];
  const batchedHooks: {
    [K in SupportedHookNames]?: [NonNullable<Plugin[K]>, Plugin][];
  } = {};

  plugins.forEach((plugin, index) => {
    const pluginName = plugin.name || `Anonymous(index: ${index})`;
    names.push(pluginName);
    R.keys(plugin).forEach((pluginProp) => {
      if (isUnsupportedHooks(pluginProp)) {
        throw new Error(
          `Failed to compose js plugins. Plugin ${pluginName} has an unsupported hook: ${pluginProp}`,
        );
      }

      if (!isPluginHookName(pluginProp)) {
        // Not hooks. Just ignore these properties
        return;
      }

      switch (pluginProp) {
        case 'buildStart': {
          const handlers = batchedHooks.buildStart ?? [];
          batchedHooks.buildStart = handlers;
          if (plugin.buildStart) {
            handlers.push([plugin.buildStart, plugin]);
          }
          break;
        }
        case 'load': {
          const handlers = batchedHooks.load ?? [];
          batchedHooks.load = handlers;
          if (plugin.load) {
            handlers.push([plugin.load, plugin]);
          }
          break;
        }
        case 'transform': {
          const handlers = batchedHooks.transform ?? [];
          batchedHooks.transform = handlers;
          if (plugin.transform) {
            handlers.push([plugin.transform, plugin]);
          }
          break;
        }
        case 'resolveId': {
          const handlers = batchedHooks.resolveId ?? [];
          batchedHooks.resolveId = handlers;
          if (plugin.resolveId) {
            handlers.push([plugin.resolveId, plugin]);
          }
          break;
        }
        case 'buildEnd': {
          const handlers = batchedHooks.buildEnd ?? [];
          batchedHooks.buildEnd = handlers;
          if (plugin.buildEnd) {
            handlers.push([plugin.buildEnd, plugin]);
          }
          break;
        }
        case 'renderChunk': {
          const handlers = batchedHooks.renderChunk ?? [];
          batchedHooks.renderChunk = handlers;
          if (plugin.renderChunk) {
            handlers.push([plugin.renderChunk, plugin]);
          }
          break;
        }
        case 'banner':
        case 'footer':
        case 'intro':
        case 'outro': {
          const hook = plugin[pluginProp];
          if (hook) {
            (batchedHooks[pluginProp] ??= []).push([hook, plugin]);
          }
          break;
        }
        case 'closeBundle': {
          const handlers = batchedHooks.closeBundle ?? [];
          batchedHooks.closeBundle = handlers;
          if (plugin.closeBundle) {
            handlers.push([plugin.closeBundle, plugin]);
          }
          break;
        }

        case 'watchChange': {
          const handlers = batchedHooks.watchChange ?? [];
          batchedHooks.watchChange = handlers;
          if (plugin.watchChange) {
            handlers.push([plugin.watchChange, plugin]);
          }
          break;
        }

        case 'closeWatcher': {
          const handlers = batchedHooks.closeWatcher ?? [];
          batchedHooks.closeWatcher = handlers;
          if (plugin.closeWatcher) {
            handlers.push([plugin.closeWatcher, plugin]);
          }
          break;
        }

        default: {
          // All known hooks should be handled above.
          // User-defined custom properties will hit this branch and it's ok. Just ignore them.
          type _ExecutiveCheck = AssertNever<typeof pluginProp>;
        }
      }
    });
  });

  const composed: Plugin = {
    name: `Composed(${names.join(', ')})`,
  };

  const createFixedPluginResolveFnMap = new Map<
    Plugin,
    (
      ctx: PluginContext,
      resolve: PluginContext['resolve'],
    ) => PluginContext['resolve']
  >();

  function applyFixedPluginResolveFn(ctx: PluginContext, plugin: Plugin) {
    const createFixedPluginResolveFn = createFixedPluginResolveFnMap.get(
      plugin,
    );

    if (createFixedPluginResolveFn) {
      ctx.resolve = createFixedPluginResolveFn(ctx, ctx.resolve.bind(ctx));
    }

    return ctx;
  }

  if (batchedHooks.resolveId) {
    const batchedHandlers = batchedHooks.resolveId;
    const handlerSymbols = batchedHandlers.map(([_handler, plugin]) =>
      Symbol(plugin.name ?? `Anonymous`)
    );
    for (
      let handlerIdx = 0;
      handlerIdx < batchedHandlers.length;
      handlerIdx++
    ) {
      const [_handler, plugin] = batchedHandlers[handlerIdx];
      const handlerSymbol = handlerSymbols[handlerIdx];
      const createFixedPluginResolveFn = (
        ctx: PluginContext,
        resolve: PluginContext['resolve'],
      ): PluginContext['resolve'] => {
        return (source, importer, rawContextResolveOptions) => {
          const contextResolveOptions: PrivatePluginContextResolveOptions =
            rawContextResolveOptions ?? {};

          if (contextResolveOptions.skipSelf) {
            contextResolveOptions[SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF] =
              handlerSymbol;
            contextResolveOptions.skipSelf = false;
          }
          return resolve(source, importer, contextResolveOptions);
        };
      };
      createFixedPluginResolveFnMap.set(plugin, createFixedPluginResolveFn);
    }

    composed.resolveId = async function(
      source,
      importer,
      rawHookResolveIdOptions,
    ) {
      const hookResolveIdOptions: PrivateResolveIdExtraOptions =
        rawHookResolveIdOptions;

      const symbolForCallerThatSkipSelf = hookResolveIdOptions
        ?.[SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF];

      for (
        let handlerIdx = 0;
        handlerIdx < batchedHandlers.length;
        handlerIdx++
      ) {
        const [handler, plugin] = batchedHandlers[handlerIdx];
        const handlerSymbol = handlerSymbols[handlerIdx];

        if (symbolForCallerThatSkipSelf === handlerSymbol) {
          continue;
        }

        const { handler: handlerFn } = normalizeHook(handler);
        const result = await handlerFn.call(
          applyFixedPluginResolveFn(this, plugin),
          source,
          importer,
          rawHookResolveIdOptions,
        );
        if (!isNullish(result)) {
          return result;
        }
      }
    };
  }

  R.keys(batchedHooks).forEach((hookName) => {
    switch (hookName) {
      case 'resolveId': {
        // It's handled above
        break;
      }
      case 'buildStart': {
        if (batchedHooks.buildStart) {
          const batchedHandlers = batchedHooks.buildStart;
          composed.buildStart = async function(options) {
            await Promise.all(
              batchedHandlers.map(([handler, plugin]) => {
                const { handler: handlerFn } = normalizeHook(handler);
                return handlerFn.call(
                  applyFixedPluginResolveFn(this, plugin),
                  options,
                );
              }),
            );
          };
        }
        break;
      }
      case 'load': {
        if (batchedHooks.load) {
          const batchedHandlers = batchedHooks.load;
          composed.load = async function(id) {
            for (const [handler, plugin] of batchedHandlers) {
              const { handler: handlerFn } = normalizeHook(handler);
              const result = await handlerFn.call(
                applyFixedPluginResolveFn(this, plugin),
                id,
              );
              if (!isNullish(result)) {
                return result;
              }
            }
          };
        }
        break;
      }
      case 'transform': {
        if (batchedHooks.transform) {
          const batchedHandlers = batchedHooks.transform;
          composed.transform = async function(initialCode, id, moduleType) {
            let code = initialCode;
            let moduleSideEffects: ModuleSideEffects | undefined = undefined;
            // TODO: we should deal with the returned sourcemap too.
            function updateOutput(
              newCode: string,
              newModuleSideEffects?: ModuleSideEffects,
            ) {
              code = newCode;
              moduleSideEffects = newModuleSideEffects ?? undefined;
            }
            for (const [handler, plugin] of batchedHandlers) {
              const { handler: handlerFn } = normalizeHook(handler);
              this.getCombinedSourcemap = () => {
                throw new Error(
                  `The getCombinedSourcemap is not implement in transform hook at composedJsPlugins`,
                );
              };
              const result = await handlerFn.call(
                applyFixedPluginResolveFn(
                  this,
                  plugin,
                ) as TransformPluginContext,
                code,
                id,
                moduleType,
              );
              if (!isNullish(result)) {
                if (typeof result === 'string') {
                  updateOutput(result);
                } else {
                  if (result.code) {
                    updateOutput(result.code, result.moduleSideEffects);
                  }
                }
              }
            }
            return {
              code,
              moduleSideEffects,
            };
          };
        }
        break;
      }
      case 'buildEnd': {
        if (batchedHooks.buildEnd) {
          const batchedHandlers = batchedHooks.buildEnd;
          composed.buildEnd = async function(err) {
            await Promise.all(
              batchedHandlers.map(([handler, plugin]) => {
                const { handler: handlerFn } = normalizeHook(handler);
                return handlerFn.call(
                  applyFixedPluginResolveFn(this, plugin),
                  err,
                );
              }),
            );
          };
        }
        break;
      }
      case 'renderChunk': {
        if (batchedHooks.renderChunk) {
          const batchedHandlers = batchedHooks.renderChunk;
          composed.renderChunk = async function(code, chunk, options, meta) {
            for (const [handler, plugin] of batchedHandlers) {
              const { handler: handlerFn } = normalizeHook(handler);
              const result = await handlerFn.call(
                applyFixedPluginResolveFn(this, plugin),
                code,
                chunk,
                options,
                meta,
              );
              if (!isNullish(result)) {
                return result;
              }
            }
          };
        }
        break;
      }
      case 'banner':
      case 'footer':
      case 'intro':
      case 'outro': {
        const hooks = batchedHooks[hookName];
        if (hooks?.length) {
          composed[hookName] = async function(chunk) {
            const ret: string[] = [];
            for (const [hook, plugin] of hooks) {
              {
                const { handler } = normalizeHook(hook);
                ret.push(
                  typeof handler === 'string'
                    ? handler
                    : await handler.call(
                      applyFixedPluginResolveFn(this, plugin),
                      chunk,
                    ),
                );
              }
            }
            return ret.join('\n');
          };
        }
        break;
      }
      case 'closeBundle': {
        if (batchedHooks.closeBundle) {
          const batchedHandlers = batchedHooks.closeBundle;
          composed.closeBundle = async function() {
            await Promise.all(
              batchedHandlers.map(([handler, plugin]) => {
                const { handler: handlerFn } = normalizeHook(handler);
                return handlerFn.call(applyFixedPluginResolveFn(this, plugin));
              }),
            );
          };
        }
        break;
      }
      case 'watchChange': {
        if (batchedHooks.watchChange) {
          const batchedHandlers = batchedHooks.watchChange;
          composed.watchChange = async function(id, event) {
            await Promise.all(
              batchedHandlers.map(([handler, plugin]) => {
                const { handler: handlerFn } = normalizeHook(handler);
                return handlerFn.call(
                  applyFixedPluginResolveFn(this, plugin),
                  id,
                  event,
                );
              }),
            );
          };
        }
        break;
      }
      case 'closeWatcher': {
        if (batchedHooks.closeWatcher) {
          const batchedHandlers = batchedHooks.closeWatcher;
          composed.closeWatcher = async function() {
            await Promise.all(
              batchedHandlers.map(([handler, plugin]) => {
                const { handler: handlerFn } = normalizeHook(handler);
                return handlerFn.call(applyFixedPluginResolveFn(this, plugin));
              }),
            );
          };
        }
        break;
      }
      default: {
        // Supported hooks should be handled above, otherwise it should be filtered out in the beginning.
        type _ExhaustiveCheck = AssertNever<typeof hookName>;
      }
    }
  });

  return composed;
}

function isComposablePlugin(plugin: RolldownPlugin): plugin is Plugin {
  if (plugin instanceof BuiltinPlugin) {
    return false;
  }

  if ('_parallel' in plugin) {
    return false;
  }

  // Check if the plugin has patterns that aren't composable
  const hasNotComposablePattern = R.keys(plugin).some((hookName) => {
    if (!isPluginHookName(hookName)) {
      // Not hooks. Just ignore these properties since they don't affect the composable pattern
      return false;
    }

    const OK_TO_COMPOSE = false;

    if (isUnsupportedHooks(hookName)) {
      return !OK_TO_COMPOSE;
    }

    if (plugin[hookName]) {
      const { meta } = normalizeHook(plugin[hookName]);
      // if `order` is specified with `pre` or `post`, it's unsafe to compose this plugin
      if (meta.order === 'pre' || meta.order === 'post') {
        return !OK_TO_COMPOSE;
      }
    }

    return OK_TO_COMPOSE;
  });

  if (hasNotComposablePattern) {
    return false;
  }

  return true;
}

export function composeJsPlugins(plugins: RolldownPlugin[]): RolldownPlugin[] {
  const newPlugins: RolldownPlugin[] = [];

  const toBeComposed: Plugin[] = [];

  plugins.forEach((plugin) => {
    if (isComposablePlugin(plugin)) {
      toBeComposed.push(plugin);
    } else {
      if (toBeComposed.length > 0) {
        if (toBeComposed.length > 1) {
          newPlugins.push(createComposedPlugin(toBeComposed));
        } else {
          // push the only plugin in toBeComposed
          newPlugins.push(toBeComposed[0]);
        }
        toBeComposed.length = 0;
      }
      // push the plugin that is not composable
      newPlugins.push(plugin);
    }
  });
  // Considering the case:
  // p = [c, c, c, c]
  // after the loop, toBeComposed = [c, c, c, c], plugins = []
  // we should consume all the toBeComposed plugins at the end
  if (toBeComposed.length > 0) {
    if (toBeComposed.length > 1) {
      newPlugins.push(createComposedPlugin(toBeComposed));
    } else {
      newPlugins.push(toBeComposed[0]);
    }
    toBeComposed.length = 0;
  }

  return newPlugins;
}
