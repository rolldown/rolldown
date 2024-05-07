import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'

import type { Plugin } from './index'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import { isEmptySourcemapFiled, transformModuleInfo } from '../utils'
import path from 'path'
import { SourceMapInputObject } from '../types/sourcemap'
import { transformPluginContext } from './plugin-context'

export function bindingifyBuildStart(
  options: NormalizedInputOptions,
  hook?: Plugin['buildStart'],
): BindingPluginOptions['buildStart'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx) => {
    await handler.call(transformPluginContext(ctx), options)
  }
}

export function bindingifyBuildEnd(
  hook?: Plugin['buildEnd'],
): BindingPluginOptions['buildEnd'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, err) => {
    await handler.call(
      transformPluginContext(ctx),
      err ? new Error(err) : undefined,
    )
  }
}

export function bindingifyResolveId(
  hook?: Plugin['resolveId'],
): BindingPluginOptions['resolveId'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, specifier, importer, options) => {
    const ret = await handler.call(
      transformPluginContext(ctx),
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

export function bindingifyResolveDynamicImport(
  hook?: Plugin['resolveDynamicImport'],
): BindingPluginOptions['resolveDynamicImport'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, specifier, importer) => {
    const ret = await handler.call(
      transformPluginContext(ctx),
      specifier,
      importer ?? undefined,
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

  return async (ctx, code, id) => {
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

    return {
      code: ret.code,
      map: typeof ret.map === 'object' ? JSON.stringify(ret.map) : ret.map,
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

  return async (ctx, id) => {
    const ret = await handler.call(transformPluginContext(ctx), id)

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

export function bindingifyModuleParsed(
  hook?: Plugin['moduleParsed'],
): BindingPluginOptions['moduleParsed'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, moduleInfo) => {
    await handler.call(
      transformPluginContext(ctx),
      transformModuleInfo(moduleInfo),
    )
  }
}
