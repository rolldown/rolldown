import type {
  BindingHookFilter,
  BindingHookResolveIdOutput,
  BindingPluginOptions,
} from '../binding';
import { normalizeHook } from '../utils/normalize-hook';

import path from 'node:path';
import {
  bindingifySourcemap,
  type ExistingRawSourceMap,
} from '../types/sourcemap';
import { aggregateBindingErrorsIntoJsError } from '../utils/error';
import { transformModuleInfo } from '../utils/transform-module-info';
import {
  isEmptySourcemapFiled,
  normalizeTransformHookSourcemap,
} from '../utils/transform-sourcemap';
import {
  bindingifyLoadFilter,
  bindingifyResolveIdFilter,
  bindingifyTransformFilter,
} from './bindingify-hook-filter';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import {
  bindingifyPluginHookMeta,
  type PluginHookWithBindingExt,
} from './bindingify-plugin-hook-meta';
import type { PluginHooks, SourceDescription } from './index';
import { PluginContextImpl } from './plugin-context';
import { TransformPluginContextImpl } from './transform-plugin-context';

export function bindingifyBuildStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildStart']> {
  const hook = args.plugin.buildStart;
  if (!hook) {
    return {};
  }
  const { handler, meta } = normalizeHook(hook);

  return {
    plugin: async (ctx, opts) => {
      await handler.call(
        new PluginContextImpl(
          args.outputOptions,
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
          args.watchMode,
        ),
        args.pluginContextData.getInputOptions(opts),
      );
    },
    meta: bindingifyPluginHookMeta(meta),
  };
}
export function bindingifyBuildEnd(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildEnd']> {
  const hook = args.plugin.buildEnd;
  if (!hook) {
    return {};
  }
  const { handler, meta } = normalizeHook(hook);

  return {
    plugin: async (ctx, err) => {
      await handler.call(
        new PluginContextImpl(
          args.outputOptions,
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
          args.watchMode,
        ),
        err ? aggregateBindingErrorsIntoJsError(err) : undefined,
      );
    },
    meta: bindingifyPluginHookMeta(meta),
  };
}

export function bindingifyResolveId(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<
  BindingPluginOptions['resolveId'],
  BindingHookFilter | undefined
> {
  const hook = args.plugin.resolveId as unknown as PluginHooks['resolveId'];
  if (!hook) {
    return {};
  }
  const { handler, meta, options } = normalizeHook(hook);

  return {
    plugin: async (ctx, specifier, importer, extraOptions) => {
      const contextResolveOptions = extraOptions.custom != null
        ? args.pluginContextData.getSavedResolveOptions(
          extraOptions.custom,
        )
        : undefined;

      const ret = await handler.call(
        new PluginContextImpl(
          args.outputOptions,
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
          args.watchMode,
        ),
        specifier,
        importer ?? undefined,
        {
          ...extraOptions,
          custom: contextResolveOptions?.custom,
        },
      );
      if (ret == null) {
        return;
      }
      if (ret === false) {
        return {
          id: specifier,
          external: true,
          normalizeExternalId: true,
        };
      }
      if (typeof ret === 'string') {
        return { id: ret, normalizeExternalId: false };
      }

      // Make sure the `moduleSideEffects` is update to date
      let exist = args.pluginContextData.updateModuleOption(ret.id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects ?? null,
        invalidate: false,
      });

      return {
        id: ret.id,
        external: ret.external,
        normalizeExternalId: false,
        moduleSideEffects: exist.moduleSideEffects ?? undefined,
        packageJsonPath: ret.packageJsonPath,
      };
    },
    meta: bindingifyPluginHookMeta(meta),
    filter: bindingifyResolveIdFilter(options.filter),
  };
}

export function bindingifyResolveDynamicImport(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveDynamicImport']> {
  const hook = args.plugin.resolveDynamicImport;
  if (!hook) {
    return {};
  }
  const { handler, meta } = normalizeHook(hook);

  return {
    plugin: async (ctx, specifier, importer) => {
      const ret = await handler.call(
        new PluginContextImpl(
          args.outputOptions,
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
          args.watchMode,
        ),
        specifier,
        importer ?? undefined,
      );
      if (ret == null) {
        return;
      }
      if (ret === false) {
        return {
          id: specifier,
          external: true,
        };
      }
      if (typeof ret === 'string') {
        return {
          id: ret,
        };
      }

      const result: BindingHookResolveIdOutput = {
        id: ret.id,
        external: ret.external,
        packageJsonPath: ret.packageJsonPath,
      };

      if (ret.moduleSideEffects !== null) {
        result.moduleSideEffects = ret.moduleSideEffects;
      }

      args.pluginContextData.updateModuleOption(ret.id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
        invalidate: false,
      });

      return result;
    },
    meta: bindingifyPluginHookMeta(meta),
  };
}

export function bindingifyTransform(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<
  BindingPluginOptions['transform'],
  BindingHookFilter | undefined
> {
  const hook = args.plugin.transform;
  if (!hook) {
    return {};
  }
  const { handler, meta, options } = normalizeHook(hook);

  return {
    plugin: async (ctx, code, id, meta) => {
      const ret = await handler.call(
        new TransformPluginContextImpl(
          args.outputOptions,
          ctx.inner(),
          args.plugin,
          args.pluginContextData,
          ctx,
          id,
          code,
          args.onLog,
          args.logLevel,
          args.watchMode,
        ),
        code,
        id,
        meta,
      );

      if (ret == null) {
        return undefined;
      }

      if (typeof ret === 'string') {
        return { code: ret };
      }

      let moduleOption = args.pluginContextData.updateModuleOption(id, {
        meta: ret.meta ?? {},
        moduleSideEffects: ret.moduleSideEffects ?? null,
        invalidate: false,
      });

      return {
        code: ret.code,
        map: bindingifySourcemap(
          normalizeTransformHookSourcemap(id, code, ret.map),
        ),
        moduleSideEffects: moduleOption.moduleSideEffects ?? undefined,
        moduleType: ret.moduleType,
      };
    },
    meta: bindingifyPluginHookMeta(meta),
    filter: bindingifyTransformFilter(options.filter),
  };
}

export function bindingifyLoad(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<
  BindingPluginOptions['load'],
  BindingHookFilter | undefined
> {
  const hook = args.plugin.load;
  if (!hook) {
    return {};
  }
  const { handler, meta, options } = normalizeHook(hook);

  return {
    plugin: async (ctx, id) => {
      const ret = await handler.call(
        new PluginContextImpl(
          args.outputOptions,
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
          args.watchMode,
          id,
        ),
        id,
      );

      if (ret == null) {
        return;
      }

      if (typeof ret === 'string') {
        return { code: ret };
      }

      let moduleOption = args.pluginContextData.updateModuleOption(id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects ?? null,
        invalidate: false,
      });

      let map = preProcessSourceMap(ret, id);

      return {
        code: ret.code,
        map: bindingifySourcemap(map),
        moduleType: ret.moduleType,
        moduleSideEffects: moduleOption.moduleSideEffects ?? undefined,
      };
    },
    meta: bindingifyPluginHookMeta(meta),
    filter: bindingifyLoadFilter(options.filter),
  };
}

function preProcessSourceMap(
  ret: SourceDescription,
  id: string,
): ExistingRawSourceMap | null | undefined {
  if (!ret.map) {
    return;
  }
  let map = typeof ret.map === 'object'
    ? ret.map
    : (JSON.parse(ret.map) as ExistingRawSourceMap);
  if (!isEmptySourcemapFiled(map.sources)) {
    // normalize original sourcemap sources
    // Port form https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts#L180-L188.
    const directory = path.dirname(id) || '.';
    const sourceRoot = map.sourceRoot || '.';
    map.sources = map.sources!.map((source) =>
      path.resolve(directory, sourceRoot, source!)
    );
  }
  return map;
}

export function bindingifyModuleParsed(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['moduleParsed']> {
  const hook = args.plugin.moduleParsed;
  if (!hook) {
    return {};
  }
  const { handler, meta } = normalizeHook(hook);

  return {
    plugin: async (ctx, moduleInfo) => {
      await handler.call(
        new PluginContextImpl(
          args.outputOptions,
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
          args.watchMode,
        ),
        transformModuleInfo(
          moduleInfo,
          args.pluginContextData.getModuleOption(moduleInfo.id),
        ),
      );
    },
    meta: bindingifyPluginHookMeta(meta),
  };
}
