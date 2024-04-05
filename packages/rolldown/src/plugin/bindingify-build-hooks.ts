import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'

import type { Plugin } from './index'
import { RolldownNormalizedInputOptions } from '../options/input-options'
import { isEmptySourcemapFiled, transformSourcemap } from '../utils'

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

    // TODO(underfin) move the logic to rust
    // If sourcemap hasn't `sourcesContent` and `sources`, using original code to fill it.
    if (ret.map && typeof ret.map === 'object') {
      if (isEmptySourcemapFiled(ret.map.sourcesContent)) {
        ret.map.sourcesContent = [code]
      }
      if (isEmptySourcemapFiled(ret.map.sources)) {
        ret.map.sources = [id]
      }
    }

    return {
      code: ret.code,
      map: transformSourcemap(ret.map),
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

    return {
      code: ret.code,
      map: transformSourcemap(ret.map),
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

    return {
      code: ret,
    }
  }
}
