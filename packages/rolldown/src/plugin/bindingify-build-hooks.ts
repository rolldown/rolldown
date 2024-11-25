import { normalizeHook } from '../utils/normalize-hook'
import type {
  BindingHookResolveIdOutput,
  BindingPluginOptions,
} from '../binding'

import type {
  hookFilterExtension,
  PluginHooks,
  PrivateResolveIdExtraOptions,
  SourceDescription,
} from './index'
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
import type { BindingifyPluginArgs } from './bindingify-plugin'
import { NormalizedInputOptionsImpl } from '../options/normalized-input-options'

export function bindingifyBuildStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildStart']> {
  const hook = args.plugin.buildStart
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, opts) => {
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        new NormalizedInputOptionsImpl(opts, args.onLog),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
export function bindingifyBuildEnd(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildEnd']> {
  const hook = args.plugin.buildEnd
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, err) => {
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        err ? new Error(err) : undefined,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyResolveId(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<
  BindingPluginOptions['resolveId'],
  hookFilterExtension<'transform'>
> {
  const hook = args.plugin.resolveId as unknown as PluginHooks['resolveId']
  if (!hook) {
    return {}
  }
  const { handler, meta, options } = normalizeHook(hook)

  return {
    plugin: async (ctx, specifier, importer, extraOptions) => {
      const contextResolveOptions =
        extraOptions.custom != null
          ? (args.pluginContextData.getSavedResolveOptions(
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
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        specifier,
        importer ?? undefined,
        newExtraOptions,
      )
      if (ret == null) {
        return
      }
      if (ret === false) {
        return {
          id: specifier,
          external: true,
        }
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

      args.pluginContextData.updateModuleOption(ret.id, {
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
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveDynamicImport']> {
  const hook = args.plugin.resolveDynamicImport
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, specifier, importer) => {
      const ret = await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        specifier,
        importer ?? undefined,
      )
      if (ret == null) {
        return
      }
      if (ret === false) {
        return {
          id: specifier,
          external: true,
        }
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
        result.sideEffects = bindingifySideEffects(ret.moduleSideEffects)
      }

      args.pluginContextData.updateModuleOption(ret.id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
      })

      return result
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyTransform(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['transform']> {
  const hook = args.plugin.transform
  if (!hook) {
    return {}
  }
  const { handler, meta, options } = normalizeHook(hook)

  return {
    plugin: async (ctx, code, id, meta) => {
      const ret = await handler.call(
        new TransformPluginContext(
          ctx.inner(),
          args.plugin,
          args.pluginContextData,
          ctx,
          id,
          code,
          args.onLog,
          args.logLevel,
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

      args.pluginContextData.updateModuleOption(id, {
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
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['load']> {
  const hook = args.plugin.load
  if (!hook) {
    return {}
  }
  const { handler, meta, options } = normalizeHook(hook)

  return {
    plugin: async (ctx, id) => {
      const ret = await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        id,
      )

      if (ret == null) {
        return
      }

      if (typeof ret === 'string') {
        return { code: ret }
      }

      let map = preProcessSourceMap(ret, id)

      const result = {
        code: ret.code,
        map: map !== undefined ? bindingifySourcemap(map) : undefined,
        moduleType: ret.moduleType,
      }

      if (ret.moduleSideEffects !== null) {
        // @ts-ignore TODO The typing should import from binding
        result.sideEffects = bindingifySideEffects(ret.moduleSideEffects)
      }

      args.pluginContextData.updateModuleOption(id, {
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

function preProcessSourceMap(
  ret: SourceDescription,
  id: string,
): ExistingRawSourceMap | null | undefined {
  if (!ret.map) {
    return
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
  return map
}

export function bindingifyModuleParsed(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['moduleParsed']> {
  const hook = args.plugin.moduleParsed
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, moduleInfo) => {
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        transformModuleInfo(
          moduleInfo,
          args.pluginContextData.moduleOptionMap.get(moduleInfo.id)!,
        ),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
