import type { Plugin, NormalizedInputOptions } from '../rollup-types'
import type {
  PluginOptions,
  SourceResult,
  ResolveIdResult,
  FilterIdResult,
} from '@rolldown/node-binding'
import { unimplemented } from '../utils'
import { RolldownPlugin } from './input-options'

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function createBuildPluginAdapter(
  plugin: RolldownPlugin,
  options: NormalizedInputOptions,
): PluginOptions {
  return {
    name: plugin.name ?? 'unknown',
    buildStart: buildStart(plugin.buildStart, options),
    resolveId: resolveId(plugin.resolveId),
    filterId: filterId(plugin.filterId),
    load: load(plugin.load),
    transform: transform(plugin.transform),
    buildEnd: buildEnd(plugin.buildEnd),
  }
}

function buildStart(
  hook: Plugin['buildStart'],
  options: NormalizedInputOptions,
) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async () => {
      try {
        // Here use `Object.freeze` to prevent plugin from modifying the options.
        await hook.call({} as any, Object.freeze(options))
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function buildEnd(hook: Plugin['buildEnd']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (e: string) => {
      try {
        await hook.call({} as any, e ? new Error(e) : undefined)
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function transform(hook: Plugin['transform']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (
      code: string,
      id: string,
    ): Promise<undefined | SourceResult> => {
      try {
        // TODO: Need to investigate how to pass context to plugin.
        const value = await hook.call({} as any, code, id)
        if (value === undefined || value === null) {
          return
        }
        if (typeof value === 'string') {
          return { code: value }
        }
        if (value.code === undefined) {
          return
        }
        // TODO other filed
        return { code: value.code }
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function resolveId(hook: Plugin['resolveId']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (
      source: string,
      importer?: string,
      options?: any,
    ): Promise<undefined | ResolveIdResult> => {
      try {
        const value = await hook.call(
          {} as any,
          source,
          importer ? importer : undefined,
          options,
        )
        if (value === undefined || value === null) {
          return
        }
        if (typeof value === 'string') {
          return { id: value }
        }
        if (value === false) {
          return { id: source, external: true }
        }
        if (value.external === 'absolute' || value.external === 'relative') {
          throw new Error(
            `External module type {${value.external}} is not supported yet.`,
          )
        }
        // TODO other filed
        return value as ResolveIdResult
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function load(hook: Plugin['load']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (id: string): Promise<undefined | SourceResult> => {
      try {
        const value = await hook.call({} as any, id)
        if (value === undefined || value === null) {
          return
        }
        if (typeof value === 'string') {
          return { code: value }
        }
        if (value.code === undefined) {
          return
        }
        // TODO other filed
        return { code: value.code }
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function filterId(hook: PluginOptions['filterId']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return (): FilterIdResult => {
      try {
        const value = hook.call({} as any)

        // We must do the conversion here since napi does not support
        // unions (enums) on the Rust side.
        if (Array.isArray(value)) {
          const result: FilterIdResult = {
            include: [],
            exclude: [],
          }

          for (const item of value) {
            if (item.startsWith('!')) {
              result.exclude.push(item.slice(1))
            } else {
              result.include.push(item)
            }
          }

          return result
        }

        return value
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}
