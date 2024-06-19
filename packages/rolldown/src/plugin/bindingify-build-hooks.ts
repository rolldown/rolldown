import { normalizeHook } from '../utils/normalize-hook'
import type {
  BindingHookLoadOutput,
  BindingHookResolveIdOutput,
  BindingPluginOptions,
} from '../binding'

import type { Plugin } from './index'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import { isEmptySourcemapFiled } from '../utils/transform-sourcemap'
import { transformModuleInfo } from '../utils/transform-module-info'
import path from 'node:path'
import { bindingifySourcemap, ExistingRawSourceMap } from '../types/sourcemap'
import { PluginContext } from './plugin-context'
import { TransformPluginContext } from './transfrom-plugin-context'
import { bindingifySideEffects } from '../utils/transform-side-effects'

export function bindingifyBuildStart(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['buildStart'] {
  const hook = plugin.buildStart
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx) => {
    await handler.call(new PluginContext(options, ctx, plugin), options)
  }
}

export function bindingifyBuildEnd(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['buildEnd'] {
  const hook = plugin.buildEnd
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, err) => {
    await handler.call(
      new PluginContext(options, ctx, plugin),
      err ? new Error(err) : undefined,
    )
  }
}

export function bindingifyResolveId(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['resolveId'] {
  const hook = plugin.resolveId
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, specifier, importer, extraOptions) => {
    const ret = await handler.call(
      new PluginContext(options, ctx, plugin),
      specifier,
      importer ?? undefined,
      extraOptions,
    )
    if (ret == false || ret == null) {
      return
    }
    if (typeof ret === 'string') {
      return {
        id: ret,
      }
    }

    const result: BindingHookResolveIdOutput = {
      id: ret.id,
      external: ret.external,
    }

    if (ret.moduleSideEffects !== null) {
      // @ts-ignore TODO The typing should import from binding
      result.sideEffects = bindingifySideEffects(ret.moduleSideEffects)
    }

    return result
  }
}

export function bindingifyResolveDynamicImport(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['resolveDynamicImport'] {
  const hook = plugin.resolveDynamicImport
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, specifier, importer) => {
    const ret = await handler.call(
      new PluginContext(options, ctx, plugin),
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

    const result: BindingHookResolveIdOutput = {
      id: ret.id,
      external: ret.external,
    }

    if (ret.moduleSideEffects !== null) {
      // @ts-ignore TODO The typing should import from binding
      result.sideEffects = bindingifySideEffects(ret.moduleSideEffects)
    }

    return result
  }
}

export function bindingifyTransform(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['transform'] {
  const hook = plugin.transform
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, code, id) => {
    const ret = await handler.call(
      new TransformPluginContext(
        ctx,
        new PluginContext(options, ctx.inner(), plugin),
        id,
        code,
      ),
      code,
      id,
    )

    if (ret == null) {
      return undefined
    }

    if (typeof ret === 'string') {
      return { code: ret }
    }

    return {
      code: ret.code,
      map: bindingifySourcemap(ret.map),
      sideEffects: bindingifySideEffects(ret.moduleSideEffects),
    }
  }
}

export function bindingifyLoad(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['load'] {
  const hook = plugin.load
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, id) => {
    const ret = await handler.call(new PluginContext(options, ctx, plugin), id)

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
        : (JSON.parse(ret.map) as ExistingRawSourceMap)
    if (!isEmptySourcemapFiled(map.sources)) {
      // normalize original sourcemap sources
      // Port form https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts#L180-L188.
      const directory = path.dirname(id) || '.'
      const sourceRoot = map.sourceRoot || '.'
      map.sources = map.sources!.map((source) =>
        path.resolve(directory, sourceRoot, source!),
      )
    }

    const result = {
      code: ret.code,
      map: bindingifySourcemap(map),
    }

    if (ret.moduleSideEffects !== null) {
      // @ts-ignore TODO The typing should import from binding
      result.sideEffects = bindingifySideEffects(ret.moduleSideEffects)
    }

    return result
  }
}

export function bindingifyModuleParsed(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['moduleParsed'] {
  const hook = plugin.moduleParsed
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, moduleInfo) => {
    await handler.call(
      new PluginContext(options, ctx, plugin),
      transformModuleInfo(moduleInfo),
    )
  }
}
