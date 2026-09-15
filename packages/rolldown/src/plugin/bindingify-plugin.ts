import type { BindingPluginOptions } from '../binding.cjs';
import {
  bindingifyBuildEnd,
  bindingifyBuildStart,
  bindingifyLoad,
  bindingifyModuleParsed,
  bindingifyResolveDynamicImport,
  bindingifyResolveId,
  bindingifyTransform,
} from './bindingify-build-hooks';

import {
  bindingifyAddonHook,
  bindingifyAugmentChunkHash,
  bindingifyResolveFileUrl,
  bindingifyCloseBundle,
  bindingifyGenerateBundle,
  bindingifyRenderChunk,
  bindingifyRenderError,
  bindingifyRenderStart,
  bindingifyWriteBundle,
} from './bindingify-output-hooks';

import type { LogHandler } from '../log/log-handler';
import type { LogLevelOption } from '../log/logging';
import { error, logPluginError } from '../log/logs';
import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import {
  bindingifyCloseWatcher,
  bindingifyHotUpdate,
  bindingifyWatchChange,
} from './bindingify-watch-hooks';
import { extractHookUsage } from './generated/hook-usage';
import {
  measureHookCost,
  type PluginTimingsRecorder,
  type TimingOwner,
} from '../utils/plugin-timings';
import type { Plugin, RolldownPlugin } from './index';
import type { PluginWithInternalHooks } from './internal-hooks';
import type { PluginContextData } from './plugin-context-data';

export interface BindingifyPluginArgs {
  // `PluginWithInternalHooks` rather than `Plugin` so the bindingify functions
  // can read hidden hooks (see ./internal-hooks) without casting.
  plugin: PluginWithInternalHooks;
  options: InputOptions;
  outputOptions: OutputOptions;
  pluginContextData: PluginContextData;
  onLog: LogHandler;
  logLevel: LogLevelOption;
  watchMode: boolean;
  normalizedOutputPlugins: RolldownPlugin[];
}

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function bindingifyPlugin(
  plugin: Plugin,
  options: InputOptions,
  outputOptions: OutputOptions,
  pluginContextData: PluginContextData,
  normalizedOutputPlugins: RolldownPlugin[],
  onLog: LogHandler,
  logLevel: LogLevelOption,
  watchMode: boolean,
  timings: PluginTimingsRecorder | undefined,
): BindingPluginOptions {
  const args: BindingifyPluginArgs = {
    plugin,
    options,
    outputOptions,
    pluginContextData,
    onLog,
    logLevel,
    watchMode,
    normalizedOutputPlugins,
  };

  const { plugin: buildStart, meta: buildStartMeta } = bindingifyBuildStart(args);

  const {
    plugin: resolveId,
    meta: resolveIdMeta,
    filter: resolveIdFilter,
  } = bindingifyResolveId(args);

  const { plugin: resolveDynamicImport, meta: resolveDynamicImportMeta } =
    bindingifyResolveDynamicImport(args);

  const { plugin: buildEnd, meta: buildEndMeta } = bindingifyBuildEnd(args);

  const {
    plugin: transform,
    meta: transformMeta,
    filter: transformFilter,
  } = bindingifyTransform(args);

  const { plugin: moduleParsed, meta: moduleParsedMeta } = bindingifyModuleParsed(args);

  const { plugin: load, meta: loadMeta, filter: loadFilter } = bindingifyLoad(args);

  const {
    plugin: renderChunk,
    meta: renderChunkMeta,
    filter: renderChunkFilter,
  } = bindingifyRenderChunk(args);

  const { plugin: augmentChunkHash, meta: augmentChunkHashMeta } = bindingifyAugmentChunkHash(args);

  const { plugin: resolveFileUrl, meta: resolveFileUrlMeta } = bindingifyResolveFileUrl(args);

  const { plugin: renderStart, meta: renderStartMeta } = bindingifyRenderStart(args);

  const { plugin: renderError, meta: renderErrorMeta } = bindingifyRenderError(args);

  const { plugin: generateBundle, meta: generateBundleMeta } = bindingifyGenerateBundle(args);

  const { plugin: writeBundle, meta: writeBundleMeta } = bindingifyWriteBundle(args);

  const { plugin: closeBundle, meta: closeBundleMeta } = bindingifyCloseBundle(args);

  const { plugin: banner, meta: bannerMeta } = bindingifyAddonHook(args, 'banner');

  const { plugin: footer, meta: footerMeta } = bindingifyAddonHook(args, 'footer');

  const { plugin: intro, meta: introMeta } = bindingifyAddonHook(args, 'intro');

  const { plugin: outro, meta: outroMeta } = bindingifyAddonHook(args, 'outro');

  const { plugin: watchChange, meta: watchChangeMeta } = bindingifyWatchChange(args);

  const { plugin: hotUpdate, meta: hotUpdateMeta } = bindingifyHotUpdate(args);

  const { plugin: closeWatcher, meta: closeWatcherMeta } = bindingifyCloseWatcher(args);
  let hookUsage = extractHookUsage(plugin).inner();
  const result: BindingPluginOptions = {
    // The plugin name already normalized at `normalizePlugins`, see `packages/rolldown/src/utils/normalize-plugin-option.ts`
    name: plugin.name!,
    buildStart,
    buildStartMeta,
    resolveId,
    resolveIdMeta,
    // @ts-ignore
    resolveIdFilter,
    resolveDynamicImport,
    resolveDynamicImportMeta,
    buildEnd,
    buildEndMeta,
    transform,
    transformMeta,
    transformFilter,
    moduleParsed,
    moduleParsedMeta,
    load,
    loadMeta,
    loadFilter,
    renderChunk,
    renderChunkMeta,
    renderChunkFilter,
    augmentChunkHash,
    augmentChunkHashMeta,
    resolveFileUrl,
    resolveFileUrlMeta,
    renderStart,
    renderStartMeta,
    renderError,
    renderErrorMeta,
    generateBundle,
    generateBundleMeta,
    writeBundle,
    writeBundleMeta,
    closeBundle,
    closeBundleMeta,
    banner,
    bannerMeta,
    footer,
    footerMeta,
    intro,
    introMeta,
    outro,
    outroMeta,
    watchChange,
    watchChangeMeta,
    hotUpdate,
    hotUpdateMeta,
    closeWatcher,
    closeWatcherMeta,
    hookUsage,
  };
  // Keyed on the user's plugin object rather than its name: the same plugin configured
  // twice is two culprits, and `normalizePlugins` allows the duplicate name.
  return wrapHandlers(result, { key: plugin, name: result.name, kind: 'plugin' }, timings);
}

function wrapHandlers(
  plugin: BindingPluginOptions,
  owner: TimingOwner,
  timings: PluginTimingsRecorder | undefined,
): BindingPluginOptions {
  for (const hookName of [
    'buildStart',
    'resolveId',
    'resolveDynamicImport',
    'buildEnd',
    'transform',
    'moduleParsed',
    'load',
    'renderChunk',
    'augmentChunkHash',
    'resolveFileUrl',
    'renderStart',
    'renderError',
    'generateBundle',
    'writeBundle',
    'closeBundle',
    'banner',
    'footer',
    'intro',
    'outro',
    'watchChange',
    'hotUpdate',
    'closeWatcher',
  ] as const) {
    const raw = plugin[hookName] as any;
    // Measure the handler itself, inside the error wrapper, so the span covers the
    // plugin's own work rather than the wrapper's promise machinery.
    const handler = raw && measureHookCost(timings, owner, hookName, raw);
    if (handler) {
      plugin[hookName] = async (...args: any[]) => {
        try {
          return await handler(...args);
        } catch (e: any) {
          return error(
            logPluginError(e, plugin.name, {
              hook: hookName,
              id: hookName === 'transform' ? args[2] : undefined,
            }),
          );
        }
      };
    }
  }
  return plugin;
}
