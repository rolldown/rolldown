import { normalizeHook } from '../utils/normalize-hook'
import type {
  BindingHookResolveIdOutput,
  BindingPluginOptions,
} from '../binding'

import type {
  hookFilterExtension,
  Plugin,
  PluginHooks,
  PrivateResolveIdExtraOptions,
} from './index'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import { isEmptySourcemapFiled } from '../utils/transform-sourcemap'
import { transformModuleInfo } from '../utils/transform-module-info'
import path from 'node:path'
import { bindingifySourcemap, ExistingRawSourceMap } from '../types/sourcemap'
import {
  PluginContext,
  PrivatePluginContextResolveOptions,
} from './plugin-context'
import { TransformPluginContext } from './transform-plugin-context'
import { bindingifySideEffects } from '../utils/transform-side-effects'
import { PluginContextData } from './plugin-context-data'
import {
  PluginHookWithBindingExt,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'
import { SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF } from '../constants/plugin-context'
import {
  bindingifyLoadFilter,
  bindingifyResolveIdFilter,
  bindingifyTransformFilter,
} from './bindingify-hook-filter'

export function bindingifyBuildStart(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['buildStart']> {
  const hook = plugin.buildStart
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx) => {
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        options,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyBuildEnd(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['buildEnd']> {
  const hook = plugin.buildEnd
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, err) => {
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        err ? new Error(err) : undefined,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyResolveId(
  plugin: Plugin,
  normalizedOptions: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<
  BindingPluginOptions['resolveId'],
  hookFilterExtension<'transform'>
> {
  const hook = plugin.resolveId as unknown as PluginHooks['resolveId']
  if (!hook) {
    return {}
  }
  const { handler, meta, options } = normalizeHook(hook)

  return {
    plugin: async (ctx, specifier, importer, extraOptions) => {
      // `contextResolveOptions` comes from `PluginContext.resolve(.., .., options)` method if this hook is triggered by `PluginContext.resolve`.
      const contextResolveOptions =
        extraOptions.custom != null
          ? (pluginContextData.getSavedResolveOptions(
              extraOptions.custom,
            ) as PrivatePluginContextResolveOptions)
          : undefined

      const newExtraOptions: PrivateResolveIdExtraOptions = {
        ...extraOptions,
        custom: contextResolveOptions?.custom,
        [SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF]:
          contextResolveOptions?.[SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF],
      }

      const ret = await handler.call(
        new PluginContext(normalizedOptions, ctx, plugin, pluginContextData),
        specifier,
        importer ?? undefined,
        newExtraOptions,
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

      pluginContextData.updateModuleOption(ret.id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
      })

      return result
    },
    meta: bindingifyPluginHookMeta(meta),
    // @ts-ignore
    filter: bindingifyResolveIdFilter(options.filter),
  }
}

export function bindingifyResolveDynamicImport(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['resolveDynamicImport']> {
  const hook = plugin.resolveDynamicImport
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, specifier, importer) => {
      const ret = await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
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

      pluginContextData.updateModuleOption(ret.id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
      })

      return result
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyTransform(
  plugin: Plugin,
  normalizedOptions: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['transform']> {
  const hook = plugin.transform
  if (!hook) {
    return {}
  }
  const { handler, meta, options } = normalizeHook(hook)

  return {
    plugin: async (ctx, code, id, meta) => {
      const ret = await handler.call(
        new TransformPluginContext(
          normalizedOptions,
          ctx.inner(),
          plugin,
          pluginContextData,
          ctx,
          id,
          code,
        ),
        code,
        id,
        meta,
      )

      if (ret == null) {
        return undefined
      }

      if (typeof ret === 'string') {
        return { code: ret }
      }

      pluginContextData.updateModuleOption(id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
      })

      return {
        code: ret.code,
        map: bindingifySourcemap(ret.map),
        sideEffects: bindingifySideEffects(ret.moduleSideEffects),
        moduleType: ret.moduleType,
      }
    },
    meta: bindingifyPluginHookMeta(meta),
    // @ts-ignore
    filter: bindingifyTransformFilter(options.filter),
  }
}

export function bindingifyLoad(
  plugin: Plugin,
  normalized_options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['load']> {
  const hook = plugin.load
  if (!hook) {
    return {}
  }
  const { handler, meta, options } = normalizeHook(hook)

  return {
    plugin: async (ctx, id) => {
      const ret = await handler.call(
        new PluginContext(normalized_options, ctx, plugin, pluginContextData),
        id,
      )

      if (ret == null) {
        return
      }

      if (typeof ret === 'string') {
        return { code: ret }
      }

      if (!ret.map) {
        return { code: ret.code, moduleType: ret.moduleType }
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
        moduleType: ret.moduleType,
      }

      if (ret.moduleSideEffects !== null) {
        // @ts-ignore TODO The typing should import from binding
        result.sideEffects = bindingifySideEffects(ret.moduleSideEffects)
      }

      pluginContextData.updateModuleOption(id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
      })

      return result
    },
    meta: bindingifyPluginHookMeta(meta),
    // @ts-ignore
    filter: bindingifyLoadFilter(options.filter),
  }
}

export function bindingifyModuleParsed(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['moduleParsed']> {
  const hook = plugin.moduleParsed
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, moduleInfo) => {
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        transformModuleInfo(
          moduleInfo,
          pluginContextData.moduleOptionMap.get(moduleInfo.id)!,
        ),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
