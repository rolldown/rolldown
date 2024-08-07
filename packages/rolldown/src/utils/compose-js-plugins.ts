import { ModuleSideEffects, Plugin, RolldownPlugin } from '../plugin'
// import * as _ from 'lodash-es'
import { normalizeHook } from './normalize-hook'
import { isNullish } from './misc'
import { BuiltinPlugin } from '../plugin/builtin-plugin'


// FIXME: Conflict with the `skip` option in `PluginContext#resolve`. Since we can't detect it in advance,
// we have to bailout all plugins with `resolveId` hook.
const supportedHooks = new Set([
  'buildStart',
  'load',
  'transform',
  'buildEnd',
  'renderChunk'
])

function createComposedPlugin(plugins: Plugin[]): Plugin {
  // Throw errors if we try to merge plugins with unsupported hooks

  const names: string[] = []
  const batchedHooks: {
    [K in keyof Plugin]?: NonNullable<Plugin[K]>[]
  } = {}

  plugins.forEach((plugin, index) => {
    const pluginName = plugin.name || `Anonymous(index: ${index})`
    names.push(pluginName)
    ;(Object.keys(plugin) as (keyof typeof plugin)[]).forEach((pluginProp) => {
      switch (pluginProp) {
        case 'name':
        case 'api':
          break
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
        // case 'resolveId': {
        //   const handlers = batchedHooks.resolveId ?? []
        //   batchedHooks.resolveId = handlers
        //   if (plugin.resolveId) {
        //     handlers.push(plugin.resolveId)
        //   }
        //   break
        // }
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
        case 'augmentChunkHash':
        case 'resolveId':
        case 'banner':
        case 'footer':
        case 'intro':
        case 'outro':
        case 'generateBundle':
        case 'moduleParsed':
        case 'onLog':
        case 'options':
        case 'outputOptions':
        case 'renderError':
        case 'renderStart':
        case 'resolveDynamicImport':
        case 'writeBundle': {
          throw new Error(
            `Failed to compose js plugins. Plugin ${pluginName} has an unsupported hook: ${pluginProp}`,
          )
          break
        }
        default: {
          // All known hooks should be handled above. We allow plugin to have unknown properties and we just ignore them.
          const _executiveCheck: never = pluginProp
        }
      }
    })
  })

  const composed: Plugin = {
    name: `Composed(${names.join(', ')})`,
  }

  ;(Object.keys(batchedHooks) as (keyof typeof batchedHooks)[]).forEach(
    (hookName) => {
      switch (hookName) {
        case 'buildStart': {
          if (batchedHooks.buildStart) {
            const batchedHandlers = batchedHooks.buildStart
            composed.buildStart = async function (options) {
              await Promise.all(
                batchedHandlers.map((handler) => {
                  const [handlerFn, _handlerOptions] = normalizeHook(handler)
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
                const [handlerFn, _handlerOptions] = normalizeHook(handler)
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
            composed.transform = async function (initialCode, id) {
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
                const [handlerFn, _handlerOptions] = normalizeHook(handler)
                const result = await handlerFn.call(this, code, id)
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
            composed.resolveId = async function (source, importer, options) {
              for (const handler of batchedHandlers) {
                const [handlerFn, _handlerOptions] = normalizeHook(handler)
                const result = await handlerFn.call(
                  this,
                  source,
                  importer,
                  options,
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
                  const [handlerFn, _handlerOptions] = normalizeHook(handler)
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
                const [handlerFn, _handlerOptions] = normalizeHook(handler)
                const result = await handlerFn.call(this, code, chunk, options)
                if (!isNullish(result)) {
                  return result
                }
              }
            }
          }
          break
        }
        case 'name':
        case 'api':
        case 'augmentChunkHash':
        case 'banner':
        case 'footer':
        case 'intro':
        case 'outro':
        case 'generateBundle':
        case 'moduleParsed':
        case 'onLog':
        case 'options':
        case 'outputOptions':
        case 'renderError':
        case 'renderStart':
        case 'resolveDynamicImport':
        case 'writeBundle': {
          throw new Error(`Unsupported prop detected: ${hookName}`)
          break
        }
        default: {
          // All known hooks should be handled above. We allow plugin to have unknown properties and we just ignore them.
          const _executiveCheck: never = hookName
        }
      }
    },
  )

  return composed
}

function isComposablePlugin(plugin: RolldownPlugin): plugin is Plugin {
  if (plugin instanceof BuiltinPlugin) {
    return false
  }

  if ('_parallel' in plugin) {
    return false
  }

  if ('resolveId' in plugin) {
    return false
  }

  if (Object.keys(plugin).every(supportedHooks.has)) {
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
