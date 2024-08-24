import {
  ModuleSideEffects,
  Plugin,
  PrivateResolveIdExtraOptions,
  RolldownPlugin,
} from '../plugin'
import { normalizeHook } from './normalize-hook'
import { isNullish } from './misc'
import { BuiltinPlugin } from '../plugin/builtin-plugin'
import { TupleToUnion } from 'type-fest'
import * as R from 'remeda'
import { PluginHookNames } from '../constants/plugin'
import { AssertNever } from './type-assert'
import { PrivatePluginContextResolveOptions } from '../plugin/plugin-context'
import { SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF } from '../constants/plugin-context'
import { isPluginHookName } from './plugin'

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
] as const
const unsupportedHooks: Set<string> = new Set(unsupportedHookName)

type UnsupportedHookNames = TupleToUnion<typeof unsupportedHookName>
type SupportedHookNames = Exclude<PluginHookNames, UnsupportedHookNames>

function isUnsupportedHooks(
  hookName: string,
): hookName is UnsupportedHookNames {
  return unsupportedHooks.has(hookName)
}

function createComposedPlugin(plugins: Plugin[]): Plugin {
  // Throw errors if we try to merge plugins with unsupported hooks

  const names: string[] = []
  const batchedHooks: {
    [K in SupportedHookNames]?: NonNullable<Plugin[K]>[]
  } = {}

  plugins.forEach((plugin, index) => {
    const pluginName = plugin.name || `Anonymous(index: ${index})`
    names.push(pluginName)
    R.keys(plugin).forEach((pluginProp) => {
      if (isUnsupportedHooks(pluginProp)) {
        throw new Error(
          `Failed to compose js plugins. Plugin ${pluginName} has an unsupported hook: ${pluginProp}`,
        )
      }

      if (!isPluginHookName(pluginProp)) {
        // Not hooks. Just ignore these properties
        return
      }

      switch (pluginProp) {
        case 'buildStart': {
          const handlers = batchedHooks.buildStart ?? []
          batchedHooks.buildStart = handlers
          if (plugin.buildStart) {
            handlers.push(plugin.buildStart)
          }
          break
        }
        case 'load': {
          const handlers = batchedHooks.load ?? []
          batchedHooks.load = handlers
          if (plugin.load) {
            handlers.push(plugin.load)
          }
          break
        }
        case 'transform': {
          const handlers = batchedHooks.transform ?? []
          batchedHooks.transform = handlers
          if (plugin.transform) {
            handlers.push(plugin.transform)
          }
          break
        }
        case 'resolveId': {
          const handlers = batchedHooks.resolveId ?? []
          batchedHooks.resolveId = handlers
          if (plugin.resolveId) {
            handlers.push(plugin.resolveId)
          }
          break
        }
        case 'buildEnd': {
          const handlers = batchedHooks.buildEnd ?? []
          batchedHooks.buildEnd = handlers
          if (plugin.buildEnd) {
            handlers.push(plugin.buildEnd)
          }
          break
        }
        case 'renderChunk': {
          const handlers = batchedHooks.renderChunk ?? []
          batchedHooks.renderChunk = handlers
          if (plugin.renderChunk) {
            handlers.push(plugin.renderChunk)
          }
          break
        }
        case 'banner':
        case 'footer':
        case 'intro':
        case 'outro': {
          const hook = plugin[pluginProp]
          if (hook) {
            ;(batchedHooks[pluginProp] ??= []).push(hook)
          }
          break
        }
        default: {
          // All known hooks should be handled above.
          // User-defined custom properties will hit this branch and it's ok. Just ignore them.
          type _ExecutiveCheck = AssertNever<typeof pluginProp>
        }
      }
    })
  })

  const composed: Plugin = {
    name: `Composed(${names.join(', ')})`,
  }

  R.keys(batchedHooks).forEach((hookName) => {
    switch (hookName) {
      case 'buildStart': {
        if (batchedHooks.buildStart) {
          const batchedHandlers = batchedHooks.buildStart
          composed.buildStart = async function (options) {
            await Promise.all(
              batchedHandlers.map((handler) => {
                const { handler: handlerFn } = normalizeHook(handler)
                return handlerFn.call(this, options)
              }),
            )
          }
        }
        break
      }
      case 'load': {
        if (batchedHooks.load) {
          const batchedHandlers = batchedHooks.load
          composed.load = async function (id) {
            for (const handler of batchedHandlers) {
              const { handler: handlerFn } = normalizeHook(handler)
              const result = await handlerFn.call(this, id)
              if (!isNullish(result)) {
                return result
              }
            }
          }
        }
        break
      }
      case 'transform': {
        if (batchedHooks.transform) {
          const batchedHandlers = batchedHooks.transform
          composed.transform = async function (initialCode, id, moduleType) {
            let code = initialCode
            let moduleSideEffects: ModuleSideEffects | undefined = undefined
            // TODO: we should deal with the returned sourcemap too.
            function updateOutput(
              newCode: string,
              newModuleSideEffects?: ModuleSideEffects,
            ) {
              code = newCode
              moduleSideEffects = newModuleSideEffects ?? undefined
            }
            for (const handler of batchedHandlers) {
              const { handler: handlerFn } = normalizeHook(handler)
              const result = await handlerFn.call(this, code, id, moduleType)
              if (!isNullish(result)) {
                if (typeof result === 'string') {
                  updateOutput(result)
                } else {
                  if (result.code) {
                    updateOutput(result.code, result.moduleSideEffects)
                  }
                }
              }
            }
            return {
              code,
              moduleSideEffects,
            }
          }
        }
        break
      }
      case 'resolveId': {
        if (batchedHooks.resolveId) {
          const batchedHandlers = batchedHooks.resolveId
          const handlerSymbols = batchedHandlers.map((_handler, idx) =>
            Symbol(idx),
          )
          composed.resolveId = async function (
            source,
            importer,
            rawHookResolveIdOptions,
          ) {
            const hookResolveIdOptions: PrivateResolveIdExtraOptions =
              rawHookResolveIdOptions

            const symbolForCallerThatSkipSelf =
              hookResolveIdOptions?.[SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF]

            for (
              let handlerIdx = 0;
              handlerIdx < batchedHandlers.length;
              handlerIdx++
            ) {
              const handler = batchedHandlers[handlerIdx]
              const handlerSymbol = handlerSymbols[handlerIdx]

              if (symbolForCallerThatSkipSelf === handlerSymbol) {
                continue
              }

              const { handler: handlerFn } = normalizeHook(handler)
              const result = await handlerFn.call(
                {
                  ...this,
                  resolve: (source, importer, rawContextResolveOptions) => {
                    const contextResolveOptions: PrivatePluginContextResolveOptions =
                      rawContextResolveOptions ?? {}

                    if (contextResolveOptions.skipSelf) {
                      contextResolveOptions[
                        SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF
                      ] = handlerSymbol
                      contextResolveOptions.skipSelf = false
                    }

                    return this.resolve(source, importer, contextResolveOptions)
                  },
                },
                source,
                importer,
                rawHookResolveIdOptions,
              )
              if (!isNullish(result)) {
                return result
              }
            }
          }
        }
        break
      }
      case 'buildEnd': {
        if (batchedHooks.buildEnd) {
          const batchedHandlers = batchedHooks.buildEnd
          composed.buildEnd = async function (err) {
            await Promise.all(
              batchedHandlers.map((handler) => {
                const { handler: handlerFn } = normalizeHook(handler)
                return handlerFn.call(this, err)
              }),
            )
          }
        }
        break
      }
      case 'renderChunk': {
        if (batchedHooks.renderChunk) {
          const batchedHandlers = batchedHooks.renderChunk
          composed.renderChunk = async function (code, chunk, options) {
            for (const handler of batchedHandlers) {
              const { handler: handlerFn } = normalizeHook(handler)
              const result = await handlerFn.call(this, code, chunk, options)
              if (!isNullish(result)) {
                return result
              }
            }
          }
        }
        break
      }
      case 'banner':
      case 'footer':
      case 'intro':
      case 'outro': {
        const hooks = batchedHooks[hookName]
        if (hooks?.length) {
          composed[hookName] = async function (chunk) {
            const ret: string[] = []
            for (const hook of hooks) {
              {
                const { handler } = normalizeHook(hook)
                ret.push(
                  typeof handler === 'string'
                    ? handler
                    : await handler.call(this, chunk),
                )
              }
            }
            return ret.join('\n')
          }
        }
        break
      }
      default: {
        // Supported hooks should be handled above, otherwise it should be filtered out in the beginning.
        type _ExhaustiveCheck = AssertNever<typeof hookName>
      }
    }
  })

  return composed
}

function isComposablePlugin(plugin: RolldownPlugin): plugin is Plugin {
  if (plugin instanceof BuiltinPlugin) {
    return false
  }

  if ('_parallel' in plugin) {
    return false
  }

  // Check if the plugin has patterns that aren't composable
  const hasNotComposablePattern = R.keys(plugin).some((hookName) => {
    if (!isPluginHookName(hookName)) {
      // Not hooks. Just ignore these properties since they don't affect the composable pattern
      return false
    }

    const OK_TO_COMPOSE = false

    if (isUnsupportedHooks(hookName)) {
      return !OK_TO_COMPOSE
    }

    if (plugin[hookName]) {
      const { meta } = normalizeHook(plugin[hookName])
      // if `order` is specified with `pre` or `post`, it's unsafe to compose this plugin
      if (meta.order === 'pre' || meta.order === 'post') {
        return !OK_TO_COMPOSE
      }
    }

    return OK_TO_COMPOSE
  })

  if (hasNotComposablePattern) {
    return false
  }

  return true
}

export function composeJsPlugins(plugins: RolldownPlugin[]): RolldownPlugin[] {
  const newPlugins: RolldownPlugin[] = []

  const toBeComposed: Plugin[] = []

  plugins.forEach((plugin) => {
    if (isComposablePlugin(plugin)) {
      toBeComposed.push(plugin)
    } else {
      if (toBeComposed.length > 0) {
        if (toBeComposed.length > 1) {
          newPlugins.push(createComposedPlugin(toBeComposed))
        } else {
          // push the only plugin in toBeComposed
          newPlugins.push(toBeComposed[0])
        }
        toBeComposed.length = 0
      }
      // push the plugin that is not composable
      newPlugins.push(plugin)
    }
  })
  // Considering the case:
  // p = [c, c, c, c]
  // after the loop, toBeComposed = [c, c, c, c], plugins = []
  // we should consume all the toBeComposed plugins at the end
  if (toBeComposed.length > 0) {
    if (toBeComposed.length > 1) {
      newPlugins.push(createComposedPlugin(toBeComposed))
    } else {
      newPlugins.push(toBeComposed[0])
    }
    toBeComposed.length = 0
  }

  return newPlugins
}
