import type { Program } from '@oxc-project/types';
import path from 'node:path';
import type {
  BindingHookFilter,
  BindingHookResolveIdOutput,
  BindingPluginOptions,
} from '../binding.cjs';
import { RolldownMagicString } from '../binding-magic-string';
import { parseAst } from '../parse-ast-index';
import { bindingifySourcemap, type ExistingRawSourceMap } from '../types/sourcemap';
import { aggregateBindingErrorsIntoJsError } from '../utils/error';
import { releaseOrDefer, shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
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
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import type { PluginHooks, SourceDescription } from './index';
import { LoadPluginContextImpl } from './load-plugin-context';
import { createPluginContext } from './plugin-context';
import { TransformPluginContextImpl } from './transform-plugin-context';

// Every hook invocation marshals fresh boxes (plugin context, module info,
// normalized options, and for load/transform their specialized contexts). On
// the threadless-WASI flavor, where GC finalizers cannot be relied on, the
// wrappers release each box once its hook settles. Arguments a callback may
// legally retain are handed over as plain-data snapshots first; a retained
// plugin context used after its hook settles throws a clear post-release
// error on this flavor only.
export function bindingifyBuildStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildStart']> {
  return bindingifyHook(args.plugin.buildStart, ({ handler }) => ({
    plugin: async (ctx, opts) => {
      try {
        await handler.call(
          createPluginContext(args, ctx),
          args.pluginContextData.getInputOptions(opts),
        );
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}
export function bindingifyBuildEnd(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildEnd']> {
  return bindingifyHook(args.plugin.buildEnd, ({ handler }) => ({
    plugin: async (ctx, err) => {
      try {
        await handler.call(
          createPluginContext(args, ctx),
          err ? aggregateBindingErrorsIntoJsError(err) : undefined,
        );
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyResolveId(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveId'], BindingHookFilter | undefined> {
  const hook = args.plugin.resolveId as unknown as PluginHooks['resolveId'];
  return bindingifyHook(hook, ({ handler, options }) => ({
    plugin: async (ctx, specifier, importer, extraOptions) => {
      try {
        const contextResolveOptions =
          extraOptions.custom != null
            ? args.pluginContextData.getSavedResolveOptions(extraOptions.custom)
            : undefined;

        const ret = await handler.call(
          createPluginContext(args, ctx),
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
      } finally {
        releaseOrDefer(ctx);
      }
    },
    filter: bindingifyResolveIdFilter(options.filter),
  }));
}

export function bindingifyResolveDynamicImport(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveDynamicImport']> {
  return bindingifyHook(args.plugin.resolveDynamicImport, ({ handler }) => ({
    plugin: async (ctx, specifier, importer) => {
      try {
        const ret = await handler.call(
          createPluginContext(args, ctx),
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
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}

export function bindingifyTransform(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['transform'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.transform, ({ handler, options }) => ({
    plugin: async (ctx, code, id, meta) => {
      // Hoisted so the box `ctx.inner()` mints here can be released in the
      // `finally` alongside `ctx` itself on the threadless flavor.
      const innerCtx = ctx.inner();
      // Hoisted for the same reason: the `meta.magicString` getter below mints
      // this box lazily, and the `finally` has to be able to reach it.
      let magicStringInstance: RolldownMagicString | undefined;
      try {
        let astInstance: Program;
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
        const transformCtx = new TransformPluginContextImpl(
          args.outputOptions,
          innerCtx,
          args.plugin,
          args.pluginContextData,
          ctx,
          id,
          code,
          args.onLog,
          args.logLevel,
          args.watchMode,
        );
        const ret = await handler.call(transformCtx, code, id, meta);

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

        let normalizedCode: string | undefined = undefined;
        let map = ret.map;
        let mapHandledByNativeChannel = false;
        if (typeof ret.code === 'string') {
          normalizedCode = ret.code;
        } else if (ret.code instanceof RolldownMagicString) {
          let magicString = ret.code as RolldownMagicString;
          normalizedCode = magicString.toString();
          // If the option is not enable we should just return soucemapJsonString
          let fallbackSourcemap = ctx.sendMagicString(magicString);
          if (fallbackSourcemap != undefined) {
            map = fallbackSourcemap;
          } else {
            // `experimental.nativeMagicString`: the map is delivered natively
            // out-of-band. This must signal `null`, not `undefined` — Rust
            // reads `undefined` as `Omitted` and its empty sentinel would wipe
            // out the real map produced by the channel.
            mapHandledByNativeChannel = true;
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
      } finally {
        if (shouldEagerlyFreeOutputs()) {
          // A fire-and-forget `this.load()`/`this.resolve()` may still hold a
          // borrow on `innerCtx`, so its release goes through the tracker; the
          // specialized wrapper box `ctx` is sync-only and drops directly.
          releaseOrDefer(innerCtx);
          ctx.dropInner();
          // The source text plus its UTF-16 mapping table run ~9x the source
          // bytes. `sendMagicString` above may already have moved the string
          // itself out, but the mapping table -- the bulk of that -- stays, so
          // this still has work to do; it reports `freed: false` instead of
          // throwing once there is nothing left.
          magicStringInstance?.dropInner();
        }
      }
    },
    filter: bindingifyTransformFilter(options.filter),
  }));
}

export function bindingifyLoad(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['load'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.load, ({ handler, options }) => ({
    plugin: async (ctx, id) => {
      // Hoisted so the box `ctx.inner()` mints here can be released in the
      // `finally` alongside `ctx` itself on the threadless flavor.
      const innerCtx = ctx.inner();
      try {
        const ret = await handler.call(
          new LoadPluginContextImpl(
            args.outputOptions,
            innerCtx,
            args.plugin,
            args.pluginContextData,
            ctx,
            id,
            args.onLog,
            args.logLevel,
            args.watchMode,
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
      } finally {
        if (shouldEagerlyFreeOutputs()) {
          // Tracker release for `innerCtx`, direct drop for the sync-only
          // wrapper box (see the load hook above).
          releaseOrDefer(innerCtx);
          ctx.dropInner();
        }
      }
    },
    filter: bindingifyLoadFilter(options.filter),
  }));
}

function preProcessSourceMap(
  ret: SourceDescription,
  id: string,
): ExistingRawSourceMap | null | undefined {
  if (!ret.map) {
    return;
  }
  let map = typeof ret.map === 'object' ? ret.map : (JSON.parse(ret.map) as ExistingRawSourceMap);
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
      try {
        // The module-info box retains the module's full source, the largest
        // per-rebuild retention a hook argument carries, so on the threadless
        // flavor pass a plain-data snapshot and release the box up front.
        const moduleOption = args.pluginContextData.getModuleOption(moduleInfo.id);
        await handler.call(
          createPluginContext(args, ctx),
          shouldEagerlyFreeOutputs()
            ? snapshotModuleInfo(moduleInfo, moduleOption)
            : transformModuleInfo(moduleInfo, moduleOption),
        );
      } finally {
        releaseOrDefer(ctx);
      }
    },
  }));
}
