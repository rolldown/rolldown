import type { Plugin, NormalizedInputOptions } from '../rollup-types'
import type {
  PluginOptions,
  SourceResult,
  ResolveIdResult,
} from '@rolldown/node-binding'
import { unimplemented } from '../utils'

export function createBuildPluginAdapter(
  plugin: Plugin,
  options: NormalizedInputOptions,
): PluginOptions {
  return {
    name: plugin.name ?? 'unknown',
    buildStart: buildStart(plugin.buildStart, options),
    resolveId: resolveId(plugin.resolveId),
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
      // Here use `Object.freeze` to prevent plugin from modifying the options.
      await hook.call({} as any, Object.freeze(options))
    }
  }
}

function buildEnd(hook: Plugin['buildEnd']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (e: string) => {
      await hook.call({} as any, e ? new Error(e) : undefined)
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
    }
  }
}

function load(hook: Plugin['load']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (id: string): Promise<undefined | SourceResult> => {
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
    }
  }
}
