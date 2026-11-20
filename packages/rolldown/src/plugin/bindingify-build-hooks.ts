import type { Program } from '@oxc-project/types';
import path from 'node:path';
import type {
  BindingHookFilter,
  BindingHookLoadOutput,
  BindingHookResolveIdOutput,
  BindingHookTransformOutput,
  BindingPluginOptions,
  BindingTransformPluginContext,
} from '../binding.cjs';
import { RolldownMagicString } from '../binding-magic-string';
import { parseAst } from '../parse-ast-index';
import { bindingifySourcemap, type ExistingRawSourceMap } from '../types/sourcemap';
import { aggregateBindingErrorsIntoJsError } from '../utils/error';
import { discardResult, isPromiseLike } from '../utils/misc';
import { releaseOrDefer, runWithRelease, shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import {
  copySourcemapObject,
  isEmptySourcemapFiled,
  prepareTransformHookSourcemap,
} from '../utils/transform-sourcemap';
import {
  bindingifyLoadFilter,
  bindingifyResolveIdFilter,
  bindingifyTransformFilter,
} from './bindingify-hook-filter';
import type { BindingifyPluginArgs } from './bindingify-plugin';
import { bindingifyHook, type PluginHookWithBindingExt } from './bindingify-plugin-hook-meta';
import type {
  LoadResult,
  PluginHooks,
  ResolveIdResult,
  SourceDescription,
  TransformResult,
} from './index';
import { LoadPluginContextImpl } from './load-plugin-context';
import { createPluginContext } from './plugin-context';
import { TransformPluginContextImpl } from './transform-plugin-context';

// A threadless WASI host may never run GC finalizers, so each wrapper releases
// the boxes its hook received once the hook settles; arguments a callback may
// keep are passed as plain-data snapshots. Exception: Rollup lets a plugin
// keep the `buildStart`/`buildEnd` context (Vite's `vite:watch-package-data`
// does), so those boxes and their contexts go to
// `pluginContextData.retainContextBox` and drain when the build settles. A
// context kept from a per-module hook throws after release, on this flavor only.
export function bindingifyBuildStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildStart']> {
  return bindingifyHook(args.plugin.buildStart, ({ handler }) => ({
    plugin: (ctx, opts) => {
      const context = createPluginContext(args, ctx);
      const load = context.load.bind(context);
      let inBuildStart = true;
      context.load = (options) => {
        if (inBuildStart) {
          return Promise.reject(
            new Error('`this.load` is not supported in the `buildStart` hook.'),
          );
        }
        return load(options);
      };
      return runWithRelease(
        () => {
          try {
            const result = discardResult(
              handler.call(context, args.pluginContextData.getInputOptions(opts)),
            );
            if (isPromiseLike(result)) {
              return Promise.resolve(result).finally(() => {
                inBuildStart = false;
              });
            }
            inBuildStart = false;
          } catch (error) {
            inBuildStart = false;
            throw error;
          }
        },
        () => args.pluginContextData.retainContextBox(ctx, context),
      );
    },
  }));
}
export function bindingifyBuildEnd(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['buildEnd']> {
  return bindingifyHook(args.plugin.buildEnd, ({ handler }) => ({
    plugin: (ctx, err) => {
      const context = createPluginContext(args, ctx);
      return runWithRelease(
        () =>
          discardResult(
            handler.call(context, err ? aggregateBindingErrorsIntoJsError(err) : undefined),
          ),
        () => args.pluginContextData.retainContextBox(ctx, context),
      );
    },
  }));
}

export function bindingifyResolveId(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveId'], BindingHookFilter | undefined> {
  const hook = args.plugin.resolveId as unknown as PluginHooks['resolveId'];
  return bindingifyHook(hook, ({ handler, options }) => ({
    plugin: (ctx, specifier, importer, extraOptions) =>
      runWithRelease(
        () => {
          const contextResolveOptions =
            extraOptions.custom != null
              ? args.pluginContextData.getSavedResolveOptions(extraOptions.custom)
              : undefined;

          const ret = handler.call(
            createPluginContext(args, ctx),
            specifier,
            importer ?? undefined,
            {
              ...extraOptions,
              custom: contextResolveOptions?.custom,
              attributes: extraOptions.attributes ?? {},
              importerRawId: extraOptions.importerRawId ?? importer ?? undefined,
              importerAttributes: extraOptions.importerAttributes ?? {},
            },
          );
          if (isPromiseLike(ret)) {
            return Promise.resolve(ret).then((ret) =>
              bindingifyResolveIdResult(args, ret, specifier, extraOptions.attributes),
            );
          }
          return bindingifyResolveIdResult(args, ret, specifier, extraOptions.attributes);
        },
        () => releaseOrDefer(ctx),
      ),
    filter: bindingifyResolveIdFilter(options.filter),
  }));
}

function bindingifyResolveIdResult(
  args: BindingifyPluginArgs,
  ret: ResolveIdResult,
  specifier: string,
  attributes: Record<string, string> | undefined,
): BindingHookResolveIdOutput | undefined {
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
  let exist = args.pluginContextData.updateModuleOption(
    args.pluginContextData.toModuleId(ret.id, ret.attributes ?? attributes),
    {
      meta: ret.meta || {},
      moduleSideEffects: ret.moduleSideEffects ?? null,
      invalidate: false,
    },
    true,
  );

  return {
    id: ret.id,
    external: ret.external,
    normalizeExternalId: false,
    moduleSideEffects: exist.moduleSideEffects ?? undefined,
    packageJsonPath: ret.packageJsonPath,
    attributes: ret.attributes,
  };
}

export function bindingifyResolveDynamicImport(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['resolveDynamicImport']> {
  return bindingifyHook(args.plugin.resolveDynamicImport, ({ handler }) => ({
    plugin: (ctx, specifier, importer, extraOptions) =>
      runWithRelease(
        () => {
          const attributes = extraOptions.attributes ?? {};
          const ret = handler.call(
            createPluginContext(args, ctx),
            specifier,
            importer ?? undefined,
            {
              attributes,
              importerRawId: extraOptions.importerRawId ?? importer ?? undefined,
              importerAttributes: extraOptions.importerAttributes ?? {},
            },
          );
          if (isPromiseLike(ret)) {
            return Promise.resolve(ret).then((ret) =>
              bindingifyResolveDynamicImportResult(args, ret, specifier, attributes),
            );
          }
          return bindingifyResolveDynamicImportResult(args, ret, specifier, attributes);
        },
        () => releaseOrDefer(ctx),
      ),
  }));
}

function bindingifyResolveDynamicImportResult(
  args: BindingifyPluginArgs,
  ret: ResolveIdResult,
  specifier: string,
  attributes: Record<string, string>,
): BindingHookResolveIdOutput | undefined {
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

  args.pluginContextData.updateModuleOption(
    args.pluginContextData.toModuleId(ret.id, ret.attributes ?? attributes),
    {
      meta: ret.meta || {},
      moduleSideEffects: ret.moduleSideEffects || null,
      invalidate: false,
    },
    true,
  );

  return result;
}

export function bindingifyTransform(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['transform'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.transform, ({ handler, options }) => ({
    plugin: (ctx, code, id, bindingMeta) => {
      // Hoisted so the release below can reach these boxes on threadless WASI.
      const innerCtx = ctx.inner();
      let magicStringInstance: RolldownMagicString | undefined, astInstance: Program;
      // The MagicString the hook returned (maybe the plugin's own). It is dead
      // to JavaScript after `sendMagicString`, so the release drops it too.
      let sentMagicString: RolldownMagicString | undefined;
      // Set by the release: a first `meta.magicString` mint after the hook
      // settles could never be released on threadless WASI, so it throws.
      let settled = false;
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
            if (settled) {
              throw new Error(
                'meta.magicString is no longer usable: its transform hook has already settled, and a MagicString minted now could never be released on this flavor. Read it while the hook runs.',
              );
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
      const finish = (ret: TransformResult) => {
        if (
          shouldEagerlyFreeOutputs() &&
          ret != null &&
          typeof ret !== 'string' &&
          ret.code instanceof RolldownMagicString
        ) {
          // Marked before the send, so a skipped or throwing send still releases it.
          sentMagicString = ret.code;
        }
        return bindingifyTransformResult(args, ret, ctx, id);
      };
      return runWithRelease(
        () => {
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
          const ret = handler.call(transformCtx, code, id, meta);

          if (isPromiseLike(ret)) {
            return Promise.resolve(ret).then(finish);
          }
          return finish(ret);
        },
        () => {
          settled = true;
          // A fire-and-forget `this.load()`/`this.resolve()` may still hold a
          // borrow on `innerCtx`, so its release goes through the tracker; the
          // specialized wrapper box `ctx` is sync-only and drops directly.
          releaseOrDefer(innerCtx);
          ctx.dropInner();
          // `dropInner` is idempotent, so both may be one object.
          magicStringInstance?.dropInner();
          sentMagicString?.dropInner();
        },
      );
    },
    filter: bindingifyTransformFilter(options.filter),
  }));
}

function bindingifyTransformResult(
  args: BindingifyPluginArgs,
  ret: TransformResult,
  ctx: BindingTransformPluginContext,
  id: string,
): BindingHookTransformOutput | undefined {
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
  if (typeof ret.code === 'string') {
    normalizedCode = ret.code;
  } else if (ret.code instanceof RolldownMagicString) {
    let magicString = ret.code as RolldownMagicString;
    normalizedCode = magicString.toString();
    // Rolldown generates the map from the MagicString only when the hook returns no `map`,
    // the same as in `renderChunk`. An own `map` or `map: null` is used as it is. A
    // generated map in addition to an own map would apply the edits of this step two times.
    if (ret.map === undefined) {
      // The map generated from the MagicString takes the place of `ret.map`.
      // Rust returns it here unless `experimental.nativeMagicString` and
      // sourcemaps are both enabled. Then it is generated natively and the
      // Rust side already knows about it.
      map = ctx.sendMagicString(magicString) ?? undefined;
    }
  }

  return {
    code: normalizedCode,
    // Preserve the `map: null` (intentional opt-out) vs `map: undefined`
    map:
      bindingifySourcemap(prepareTransformHookSourcemap(map)) ??
      (ret.map === null ? null : undefined),
    moduleSideEffects: moduleOption.moduleSideEffects ?? undefined,
    moduleType: ret.moduleType,
  };
}

export function bindingifyLoad(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['load'], BindingHookFilter | undefined> {
  return bindingifyHook(args.plugin.load, ({ handler, options }) => ({
    plugin: (ctx, id, bindingExtraOptions) => {
      // Hoisted so the release below can reach it on threadless WASI.
      const innerCtx = ctx.inner();
      return runWithRelease(
        () => {
          const extraOptions = bindingExtraOptions ?? { rawId: id, attributes: {} };
          const ret = handler.call(
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
            extraOptions,
          );

          if (isPromiseLike(ret)) {
            return Promise.resolve(ret).then((ret) =>
              bindingifyLoadResult(args, ret, id, extraOptions.rawId),
            );
          }
          return bindingifyLoadResult(args, ret, id, extraOptions.rawId);
        },
        () => {
          // See the transform hook's release above.
          releaseOrDefer(innerCtx);
          ctx.dropInner();
        },
      );
    },
    filter: bindingifyLoadFilter(options.filter),
  }));
}

function bindingifyLoadResult(
  args: BindingifyPluginArgs,
  ret: LoadResult,
  id: string,
  rawId: string,
): BindingHookLoadOutput | undefined {
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

  let map = preProcessSourceMap(ret, rawId);

  return {
    code: ret.code,
    map: bindingifySourcemap(map),
    moduleType: ret.moduleType,
    moduleSideEffects: moduleOption.moduleSideEffects ?? undefined,
  };
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
  if (!isEmptySourcemapFiled(map.sources) || map.sources?.[0] === null) {
    // normalize original sourcemap sources
    // Port form https://github.com/rollup/rollup/blob/master/src/utils/collapseSourcemaps.ts#L180-L188.
    const directory = path.dirname(id) || '.';
    const sourceRoot = map.sourceRoot || '.';
    map.sources = map.sources!.map((source) =>
      source === null ? null : path.resolve(directory, sourceRoot, source),
    );
  }
  return map;
}

export function bindingifyModuleParsed(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['moduleParsed']> {
  return bindingifyHook(args.plugin.moduleParsed, ({ handler }) => ({
    plugin: (ctx, moduleInfo) =>
      runWithRelease(
        () => {
          // The module-info box retains the module's full source, the largest
          // per-rebuild retention a hook argument carries, so on the threadless
          // flavor pass a plain-data snapshot and release the box up front.
          const moduleOption = args.pluginContextData.getModuleOption(moduleInfo.id);
          return discardResult(
            handler.call(
              createPluginContext(args, ctx),
              shouldEagerlyFreeOutputs()
                ? snapshotModuleInfo(moduleInfo, moduleOption)
                : transformModuleInfo(moduleInfo, moduleOption),
            ),
          );
        },
        () => releaseOrDefer(ctx),
      ),
  }));
}
