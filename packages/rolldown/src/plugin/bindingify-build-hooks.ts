import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'

import type { Plugin } from './index'
import { RolldownNormalizedInputOptions } from '../options/input-options'
import { isEmptySourcemapFiled } from '../utils'
import path from 'path'
import { SourceMapInputObject } from '../types/sourcemap'

export function bindingifyBuildStart(
  options: RolldownNormalizedInputOptions,
  hook?: Plugin['buildStart'],
): BindingPluginOptions['buildStart'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx) => {
    handler.call(ctx, options)
  }
}

export function bindingifyBuildEnd(
  hook?: Plugin['buildEnd'],
): BindingPluginOptions['buildEnd'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (err) => {
    try {
      handler.call(null, err ?? undefined)
    } catch (error) {
      console.error(error)
    }
  }
}

export function bindingifyResolveId(
  hook?: Plugin['resolveId'],
): BindingPluginOptions['resolveId'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (specifier, importer, options) => {
    const ret = await handler.call(
      null,
      specifier,
      importer ?? undefined,
      options,
    )
    if (ret == false || ret == null) {
      return
    }
    if (typeof ret === 'string') {
      return {
        id: ret,
      }
    }
    return ret
  }
}

export function bindingifyTransform(
  hook?: Plugin['transform'],
): BindingPluginOptions['transform'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (code, id) => {
    const ret = await handler.call(null, code, id)

    if (ret == null) {
      return
    }

    if (typeof ret === 'string') {
      return { code: ret }
    }

    if (!ret.map) {
      return { code: ret.code }
    }

    // TODO(underfin) move the logic to rust
    // If sourcemap hasn't `sourcesContent` and `sources`, using original code to fill it.
    let map = typeof ret.map === 'object' ? ret.map : JSON.parse(ret.map)
    if (isEmptySourcemapFiled(map.sourcesContent)) {
      map.sourcesContent = [code]
    }
    if (isEmptySourcemapFiled(map.sources)) {
      map.sources = [id]
    }

    return {
      code: ret.code,
      map: JSON.stringify(map),
    }
  }
}

export function bindingifyLoad(
  hook?: Plugin['load'],
): BindingPluginOptions['load'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (id) => {
    const ret = await handler.call(null, id)

    if (ret == null) {
      return
    }

    if (typeof ret === 'string') {
      return { code: ret }
    }

    if (!ret.map) {
      return { code: ret.code }
    }

    let map =
      typeof ret.map === 'object'
        ? ret.map
        : (JSON.parse(ret.map) as SourceMapInputObject)
    if (!isEmptySourcemapFiled(map.sources)) {
      // normalize original sourcemap sources
      // Port form https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts#L180-L188.
      const directory = path.dirname(id) || '.'
      const sourceRoot = map.sourceRoot || '.'
      map.sources = map.sources!.map((source) =>
        path.resolve(directory, sourceRoot, source!),
      )
    }

    return {
      code: ret.code,
      map: JSON.stringify(map),
    }
  }
}

export function bindingifyRenderChunk(
  hook?: Plugin['renderChunk'],
): BindingPluginOptions['renderChunk'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (code, chunk) => {
    const ret = await handler.call(null, code, chunk)

    if (ret == null) {
      return
    }

    if (typeof ret === 'string') {
      return { code: ret }
    }

    if (!ret.map) {
      return { code: ret.code }
    }

    let map =
      typeof ret.map === 'object'
        ? ret.map
        : (JSON.parse(ret.map) as SourceMapInputObject)

    return {
      code: ret.code,
      map: JSON.stringify(map),
    }
  }
}
