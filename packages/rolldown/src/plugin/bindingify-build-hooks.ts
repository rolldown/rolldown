import type { Program } from '@oxc-project/types';
import path from 'node:path';
import type {
  BindingHookFilter,
  BindingHookResolveIdOutput,
  BindingPluginContext,
  BindingPluginOptions,
} from '../binding.cjs';
import { RolldownMagicString } from '../binding-magic-string';
import { parseAst } from '../parse-ast-index';
import { bindingifySourcemap, type ExistingRawSourceMap } from '../types/sourcemap';
import { aggregateBindingErrorsIntoJsError } from '../utils/error';
import { transformModuleInfo } from '../utils/transform-module-info';
import {
  copySourcemapObject,
  isEmptySourcemapFiled,
  normalizeTransformHookSourcemap,
} from '../utils/transform-sourcemap';
import {
  bindingifyLoadFilter,
  bindingifyResolveIdFilter,
  bindingifyTransformFilter,
} from './bindingify-hook-filter';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import { callCacheableHook } from './cacheable-hook-call';
import type { ModuleOptions, PluginHooks, SourceDescription } from './index';
import { LoadPluginContextImpl } from './load-plugin-context';
import { createPluginContext } from './plugin-context';
import type { RecordedHookCall } from './plugin-context-data';
import { TransformPluginContextImpl } from './transform-plugin-context';

export function bindingifyBuildStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildStart']> {
  return bindingifyHook(args.plugin.buildStart, ({ handler }) => ({
    plugin: async (ctx, opts) => {
      await handler.call(
        createPluginContext(args, ctx),
        args.pluginContextData.getInputOptions(opts),
      );
    },
  }));
}
export function bindingifyBuildEnd(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildEnd']> {
  return bindingifyHook(args.plugin.buildEnd, ({ handler }) => ({
    plugin: async (ctx, err) => {
      await handler.call(
        createPluginContext(args, ctx),
        err ? aggregateBindingErrorsIntoJsError(err) : undefined,
      );
    },
  }));
}

export function bindingifyResolveId(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveId'], BindingHookFilter | undefined> {
  const hook = args.plugin.resolveId as unknown as PluginHooks['resolveId'];
  return bindingifyHook(hook, ({ handler, options }) => ({
    plugin: async (ctx, specifier, importer, extraOptions) => {
      const contextResolveOptions =
        extraOptions.custom != null
          ? args.pluginContextData.getSavedResolveOptions(extraOptions.custom)
          : undefined;

      const data = args.pluginContextData;
      const access = data.startModuleInfoAccess();
      const ret = await callCacheableHook(
        data,
        ctx,
        access,
        { pluginName: args.plugin.name!, hookName: 'resolveId' },
        () =>
          handler.call(createPluginContext(args, ctx, access), specifier, importer ?? undefined, {
            ...extraOptions,
            custom: contextResolveOptions?.custom,
            attributes: extraOptions.attributes ?? {},
            importerRawId: extraOptions.importerRawId ?? importer ?? undefined,
            importerAttributes: extraOptions.importerAttributes ?? {},
          }),
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

      const resolvedModuleId = data.toModuleId(ret.id, ret.attributes ?? extraOptions.attributes);
      const hookCall: RecordedHookCall = {
        pluginName: args.plugin.name!,
        hookName: 'resolveId',
      };
      if (data.isResolvedSideEffectsFromMap(ret)) {
        data.markSideEffectsPassedOn(ctx, hookCall, 'returned', resolvedModuleId);
      }
      const moduleOption = {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects ?? null,
        invalidate: false,
      };
      data.syncSavedModuleOptions(resolvedModuleId, ctx);
      data.recordModuleOptions(
        ctx,
        hookCall,
        resolvedModuleId,
        moduleOption.meta,
        moduleOption.moduleSideEffects,
      );

      // Make sure the `moduleSideEffects` is update to date
      let exist = data.updateModuleOption(resolvedModuleId, moduleOption);
      if (ret.moduleSideEffects == null && exist.moduleSideEffects != null) {
        // The value below comes from another hook call, for example a hook of the resolved
        // module. A saved resolve result would keep it when that hook returns another value.
        data.markSideEffectsPassedOn(ctx, hookCall, 'passedOn', resolvedModuleId);
      }

      return {
        id: ret.id,
        external: ret.external,
        normalizeExternalId: false,
        moduleSideEffects: exist.moduleSideEffects ?? undefined,
        packageJsonPath: ret.packageJsonPath,
        attributes: ret.attributes,
      };
    },
    filter: bindingifyResolveIdFilter(options.filter),
  }));
}

export function bindingifyResolveDynamicImport(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveDynamicImport']> {
  return bindingifyHook(args.plugin.resolveDynamicImport, ({ handler }) => ({
    plugin: async (ctx, specifier, importer, extraOptions) => {
      const attributes = extraOptions.attributes ?? {};
      const data = args.pluginContextData;
      const access = data.startModuleInfoAccess();
      const ret = await callCacheableHook(
        data,
        ctx,
        access,
        {
          pluginName: args.plugin.name!,
          hookName: 'resolveDynamicImport',
        },
        () =>
          handler.call(createPluginContext(args, ctx, access), specifier, importer ?? undefined, {
            attributes,
            importerRawId: extraOptions.importerRawId ?? importer ?? undefined,
            importerAttributes: extraOptions.importerAttributes ?? {},
          }),
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
        attributes: ret.attributes,
      };

      if (ret.moduleSideEffects !== null) {
        result.moduleSideEffects = ret.moduleSideEffects;
      }

      const resolvedModuleId = data.toModuleId(ret.id, ret.attributes ?? attributes);
      const hookCall: RecordedHookCall = {
        pluginName: args.plugin.name!,
        hookName: 'resolveDynamicImport',
      };
      if (data.isResolvedSideEffectsFromMap(ret)) {
        data.markSideEffectsPassedOn(ctx, hookCall, 'returned', resolvedModuleId);
      }
      const moduleOption = {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects || null,
        invalidate: false,
      };
      data.syncSavedModuleOptions(resolvedModuleId, ctx);
      data.recordModuleOptions(
        ctx,
        hookCall,
        resolvedModuleId,
        moduleOption.meta,
        moduleOption.moduleSideEffects,
      );
      data.updateModuleOption(resolvedModuleId, moduleOption);

      return result;
    },
  }));
}

export function bindingifyTransform(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['transform'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.transform, ({ handler, options }) => ({
    plugin: async (ctx, code, id, bindingMeta) => {
      let magicStringInstance: RolldownMagicString, astInstance: Program;
      const meta = {
        ...bindingMeta,
        rawId: bindingMeta.rawId ?? id,
        attributes: bindingMeta.attributes ?? {},
      };
      Object.defineProperties(meta, {
        magicString: {
          get() {
            if (magicStringInstance) {
              return magicStringInstance;
            }
            magicStringInstance = new RolldownMagicString(code);
            return magicStringInstance;
          },
        },
        ast: {
          get() {
            if (astInstance) {
              return astInstance;
            }
            let lang: 'js' | 'jsx' | 'tsx' | 'ts' = 'js';
            switch (meta.moduleType) {
              case 'js':
              case 'jsx':
              case 'ts':
              case 'tsx':
                lang = meta.moduleType;
                break;
              default:
                break;
            }
            astInstance = parseAst(code, {
              astType: meta.moduleType.includes('ts') ? 'ts' : 'js',
              lang,
            });
            return astInstance;
          },
        },
      });
      const context = ctx.inner();
      const access = args.pluginContextData.startModuleInfoAccess();
      const transformCtx = new TransformPluginContextImpl(
        args.outputOptions,
        context,
        args.plugin,
        args.pluginContextData,
        ctx,
        id,
        code,
        args.onLog,
        args.logLevel,
        args.watchMode,
        access,
      );
      const ret = await callCacheableHook(
        args.pluginContextData,
        context,
        access,
        { pluginName: args.plugin.name!, hookName: 'transform' },
        () => handler.call(transformCtx, code, id, meta),
      );

      if (ret == null) {
        return undefined;
      }

      if (typeof ret === 'string') {
        return { code: ret };
      }

      let moduleOption = updateModuleOptionFromHook(args, context, 'transform', id, {
        meta: ret.meta ?? {},
        moduleSideEffects: ret.moduleSideEffects ?? null,
        invalidate: false,
      });

      let normalizedCode: string | undefined = undefined;
      let map = ret.map;
      let mapHandledByNativeChannel = false;
      if (typeof ret.code === 'string') {
        normalizedCode = ret.code;
      } else if (ret.code instanceof RolldownMagicString) {
        let magicString = ret.code as RolldownMagicString;
        normalizedCode = magicString.toString();
        // Rolldown generates the map from the MagicString only when the hook returns no `map`,
        // the same as in `renderChunk`. An own `map` or `map: null` is used as it is. A
        // generated map in addition to an own map would apply the edits of this step two times.
        if (ret.map === undefined) {
          // If the option is not enable we should just return soucemapJsonString
          let fallbackSourcemap = ctx.sendMagicString(magicString);
          if (fallbackSourcemap != undefined) {
            map = fallbackSourcemap;
          } else {
            // `experimental.nativeMagicString` is enabled: the sourcemap is
            // generated natively and delivered out-of-band via the magic-string
            // channel. Signal `null` (an explicit "no map on this output object")
            // rather than `undefined`, otherwise the Rust side treats this
            // transform as a missing/broken sourcemap (`Omitted`) and the empty
            // sentinel wipes out the real map produced by the channel.
            mapHandledByNativeChannel = true;
          }
        }
      }

      return {
        code: normalizedCode,
        // Preserve the `map: null` (intentional opt-out) vs `map: undefined`
        map:
          bindingifySourcemap(normalizeTransformHookSourcemap(id, code, map)) ??
          (mapHandledByNativeChannel || ret.map === null ? null : undefined),
        moduleSideEffects: moduleOption.moduleSideEffects ?? undefined,
        moduleType: ret.moduleType,
      };
    },
    filter: bindingifyTransformFilter(options.filter),
  }));
}

export function bindingifyLoad(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['load'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.load, ({ handler, options }) => ({
    plugin: async (ctx, id, bindingExtraOptions) => {
      const extraOptions = bindingExtraOptions ?? { rawId: id, attributes: {} };
      const context = ctx.inner();
      const access = args.pluginContextData.startModuleInfoAccess();
      const ret = await callCacheableHook(
        args.pluginContextData,
        context,
        access,
        { pluginName: args.plugin.name!, hookName: 'load' },
        () =>
          handler.call(
            new LoadPluginContextImpl(
              args.outputOptions,
              context,
              args.plugin,
              args.pluginContextData,
              ctx,
              id,
              args.onLog,
              args.logLevel,
              args.watchMode,
              access,
            ),
            id,
            extraOptions,
          ),
      );

      if (ret == null) {
        return;
      }

      if (typeof ret === 'string') {
        return { code: ret };
      }

      let moduleOption = updateModuleOptionFromHook(args, context, 'load', id, {
        meta: ret.meta || {},
        moduleSideEffects: ret.moduleSideEffects ?? null,
        invalidate: false,
      });

      let map = preProcessSourceMap(ret, extraOptions.rawId);

      return {
        code: ret.code,
        map: bindingifySourcemap(map),
        moduleType: ret.moduleType,
        moduleSideEffects: moduleOption.moduleSideEffects ?? undefined,
      };
    },
    filter: bindingifyLoadFilter(options.filter),
  }));
}

/**
 * Writes the `meta` and the `moduleSideEffects` that a `load` or `transform` hook returned for
 * its module `id` into `moduleOptionMap`. With the persistent cache, it also gives them to the
 * cache, which gives them back when the hook is skipped.
 */
function updateModuleOptionFromHook(
  args: BindingifyPluginArgs,
  context: BindingPluginContext,
  hookName: 'load' | 'transform',
  id: string,
  option: ModuleOptions,
): ModuleOptions {
  const data = args.pluginContextData;
  const hookCall: RecordedHookCall = { pluginName: args.plugin.name!, hookName };
  data.syncSavedModuleOptions(id, context);
  data.recordModuleOptions(context, hookCall, id, option.meta, option.moduleSideEffects);
  const moduleOption = data.updateModuleOption(id, option, true);
  if (option.moduleSideEffects == null && moduleOption.moduleSideEffects != null) {
    data.passOnModuleSideEffects(context, hookCall, id);
  }
  return moduleOption;
}

function preProcessSourceMap(
  ret: SourceDescription,
  id: string,
): ExistingRawSourceMap | null | undefined {
  if (!ret.map) {
    return;
  }
  // Work on a copy, so the write to `sources` below does not change the plugin's object.
  let map =
    typeof ret.map === 'object'
      ? copySourcemapObject(ret.map)
      : (JSON.parse(ret.map) as ExistingRawSourceMap);
  if (!isEmptySourcemapFiled(map.sources)) {
    // normalize original sourcemap sources
    // Port form https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts#L180-L188.
    const directory = path.dirname(id) || '.';
    const sourceRoot = map.sourceRoot || '.';
    map.sources = map.sources!.map((source) => path.resolve(directory, sourceRoot, source!));
  }
  return map;
}

export function bindingifyModuleParsed(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['moduleParsed']> {
  return bindingifyHook(args.plugin.moduleParsed, ({ handler }) => ({
    plugin: async (ctx, moduleInfo) => {
      const id = moduleInfo.id;
      args.pluginContextData.syncSavedModuleOptionsFromInfo(id, moduleInfo);
      const option = args.pluginContextData.getModuleOption(id);
      await handler.call(createPluginContext(args, ctx), transformModuleInfo(moduleInfo, option));
    },
  }));
}
