import type { Plugin, NormalizedInputOptions } from '../rollup-types'
import type {
  PluginOptions,
  SourceResult,
  ResolveIdResult,
  RenderedChunk,
  HookRenderChunkOutput,
  Outputs,
} from '@rolldown/node-binding'
import { transformToOutputBundle, unimplemented, transformSourcemap } from '../utils'

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
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
    renderChunk: renderChunk(plugin.renderChunk),
    generateBundle: generateBundle(plugin.generateBundle),
    writeBundle: writeBundle(plugin.writeBundle),
  }
}

function writeBundle(hook: Plugin['writeBundle']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (outputs: Outputs) => {
      try {
        // TODO outputOptions
        await hook.call({} as any, {} as any, transformToOutputBundle(outputs))
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function generateBundle(hook: Plugin['generateBundle']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (outputs: Outputs, isWrite: boolean) => {
      try {
        // TODO outputOptions
        await hook.call(
          {} as any,
          {} as any,
          transformToOutputBundle(outputs),
          isWrite,
        )
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}

function renderChunk(hook: Plugin['renderChunk']) {
  if (hook) {
    if (typeof hook !== 'function') {
      return unimplemented()
    }
    return async (
      code: string,
      chunk: RenderedChunk,
    ): Promise<undefined | HookRenderChunkOutput> => {
      try {
        let renderedChunk = Object.assign(
          {
            get name() {
              return unimplemented()
            },
            get dynamicImports() {
              return unimplemented()
            },
            get imports() {
              return unimplemented()
            },
            get implicitlyLoadedBefore() {
              return unimplemented()
            },
            get importedBindings() {
              return unimplemented()
            },
            get isImplicitEntry() {
              return unimplemented()
            },
            get referencedFiles() {
              return unimplemented()
            },
            type: 'chunk' as const,
          },
          chunk,
          {
            get modules() {
              return Object.fromEntries(
                Object.entries(chunk.modules).map(([key, value]) => [
                  key,
                  Object.assign(
                    {
                      get code() {
                        return unimplemented()
                      },
                    },
                    value,
                  ),
                ]),
              )
            },
            get facadeModuleId() {
              return chunk.facadeModuleId || null
            },
          },
        )
        // TODO options and meta
        const value = await hook.call(
          {} as any,
          code,
          renderedChunk,
          {} as any,
          {} as any,
        )
        if (value === undefined || value === null) {
          return
        }
        if (typeof value === 'string') {
          return { code: value }
        }
        if (typeof value === 'object') {
          // TODO other filed
          return { code: value.code }
        }
      } catch (error) {
        console.error(error)
        throw error
      }
    }
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
        return { code: value.code, map: transformSourcemap(value.map) }
      } catch (error) {
        console.error(error)
        throw error
      }
    }
  }
}
