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
  bindingifyAugmentChunkHash,
  bindingifyBanner,
  bindingifyCloseBundle,
  bindingifyFooter,
  bindingifyGenerateBundle,
  bindingifyIntro,
  bindingifyOutro,
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
import type { TypeAssert } from '../types/assert';
import type { CloseCallbackScope } from '../utils/close-callback-scope';
import { bindingifyCloseWatcher, bindingifyWatchChange } from './bindingify-watch-hooks';
import { extractHookUsage, HookUsageKind } from './generated/hook-usage';
import type { Plugin, RolldownPlugin } from './index';
import type { PluginContextData } from './plugin-context-data';

export interface BindingifyPluginArgs {
  plugin: Plugin;
  options: InputOptions;
  outputOptions: OutputOptions;
  pluginContextData: PluginContextData;
  onLog: LogHandler;
  logLevel: LogLevelOption;
  watchMode: boolean;
  closeCallbackScope?: CloseCallbackScope;
  configWatchHooks: boolean;
  normalizedOutputPlugins: RolldownPlugin[];
  runBuildCallback?: BuildCallbackRunner;
}

export type BuildCallbackRunner = <T>(callback: () => T, callbackName?: string) => T;

type BindingPluginCallbackName = {
  [K in keyof BindingPluginOptions]-?: NonNullable<BindingPluginOptions[K]> extends (
    ...args: any[]
  ) => any
    ? K
    : never;
}[keyof BindingPluginOptions];

const BUILD_CALLBACK_HOOK_NAMES = [
  'buildStart',
  'resolveId',
  'resolveDynamicImport',
  'buildEnd',
  'transform',
  'moduleParsed',
  'load',
  'renderChunk',
  'augmentChunkHash',
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
  'closeWatcher',
] as const satisfies readonly BindingPluginCallbackName[];

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
  closeCallbackScope?: CloseCallbackScope,
  configWatchHooks: boolean = true,
  runBuildCallback?: BuildCallbackRunner,
): BindingPluginOptions {
  const args: BindingifyPluginArgs = {
    plugin,
    options,
    outputOptions,
    pluginContextData,
    onLog,
    logLevel,
    watchMode,
    closeCallbackScope,
    configWatchHooks,
    normalizedOutputPlugins,
    runBuildCallback,
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

  const { plugin: renderStart, meta: renderStartMeta } = bindingifyRenderStart(args);

  const { plugin: renderError, meta: renderErrorMeta } = bindingifyRenderError(args);

  const { plugin: generateBundle, meta: generateBundleMeta } = bindingifyGenerateBundle(args);

  const { plugin: writeBundle, meta: writeBundleMeta } = bindingifyWriteBundle(args);

  const { plugin: closeBundle, meta: closeBundleMeta } = bindingifyCloseBundle(args);

  const { plugin: banner, meta: bannerMeta } = bindingifyBanner(args);

  const { plugin: footer, meta: footerMeta } = bindingifyFooter(args);

  const { plugin: intro, meta: introMeta } = bindingifyIntro(args);

  const { plugin: outro, meta: outroMeta } = bindingifyOutro(args);

  const { plugin: watchChange, meta: watchChangeMeta } = bindingifyWatchChange(args);

  const { plugin: closeWatcher, meta: closeWatcherMeta } = configWatchHooks
    ? bindingifyCloseWatcher(args)
    : {};
  let hookUsage = extractHookUsage(plugin).inner();
  if (!configWatchHooks) {
    hookUsage &= ~HookUsageKind.closeWatcher;
  }
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
    closeWatcher,
    closeWatcherMeta,
    hookUsage,
  };
  return wrapHandlers(result, runBuildCallback);
}

function wrapHandlers(
  plugin: BindingPluginOptions,
  runBuildCallback: BuildCallbackRunner | undefined,
): BindingPluginOptions {
  for (const hookName of BUILD_CALLBACK_HOOK_NAMES) {
    const handler = plugin[hookName] as any;
    if (handler) {
      plugin[hookName] = (...args: any[]) => {
        const invoke = async () => {
          try {
            return await handler(...args);
          } catch (e: any) {
            const pluginError = logPluginError(e, plugin.name, {
              hook: hookName,
              id: hookName === 'transform' ? args[2] : undefined,
            });
            if (hookName === 'closeBundle') {
              throw pluginError;
            }
            return error(pluginError);
          }
        };
        return runBuildCallback ? runBuildCallback(invoke, hookName) : invoke();
      };
    }
  }
  return plugin;
}

function _assertCallbackInventory() {
  type MissingCallbacks = Exclude<
    BindingPluginCallbackName,
    (typeof BUILD_CALLBACK_HOOK_NAMES)[number]
  >;
  type _ = TypeAssert<[MissingCallbacks] extends [never] ? true : false>;
}
