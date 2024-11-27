import type { BindingPluginOptions } from '../binding'
import {
  bindingifyBuildEnd,
  bindingifyBuildStart,
  bindingifyLoad,
  bindingifyModuleParsed,
  bindingifyResolveDynamicImport,
  bindingifyResolveId,
  bindingifyTransform,
} from './bindingify-build-hooks'

import {
  bindingifyRenderStart,
  bindingifyRenderChunk,
  bindingifyGenerateBundle,
  bindingifyWriteBundle,
  bindingifyRenderError,
  bindingifyAugmentChunkHash,
  bindingifyBanner,
  bindingifyFooter,
  bindingifyIntro,
  bindingifyOutro,
  bindingifyCloseBundle,
} from './bindingify-output-hooks'

import type { Plugin } from './index'
import type { OutputOptions } from '../options/output-options'
import { PluginContextData } from './plugin-context-data'
import {
  bindingifyCloseWatcher,
  bindingifyWatchChange,
} from './bindingify-watch-hooks'
import { error, logPluginError } from '../log/logs'
import type { LogHandler, LogLevelOption } from '../rollup'
import type { InputOptions } from '../options/input-options'

export interface BindingifyPluginArgs {
  plugin: Plugin
  options: InputOptions
  outputOptions: OutputOptions
  pluginContextData: PluginContextData
  onLog: LogHandler
  logLevel: LogLevelOption
}

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function bindingifyPlugin(
  plugin: Plugin,
  options: InputOptions,
  outputOptions: OutputOptions,
  pluginContextData: PluginContextData,
  onLog: LogHandler,
  logLevel: LogLevelOption,
): BindingPluginOptions {
  const args: BindingifyPluginArgs = {
    plugin,
    options,
    outputOptions,
    pluginContextData,
    onLog,
    logLevel,
  }

  const { plugin: buildStart, meta: buildStartMeta } =
    bindingifyBuildStart(args)

  const {
    plugin: resolveId,
    meta: resolveIdMeta,
    filter: resolveIdFilter,
  } = bindingifyResolveId(args)

  const { plugin: resolveDynamicImport, meta: resolveDynamicImportMeta } =
    bindingifyResolveDynamicImport(args)

  const { plugin: buildEnd, meta: buildEndMeta } = bindingifyBuildEnd(args)

  const {
    plugin: transform,
    meta: transformMeta,
    filter: transformFilter,
  } = bindingifyTransform(args)

  const { plugin: moduleParsed, meta: moduleParsedMeta } =
    bindingifyModuleParsed(args)

  const {
    plugin: load,
    meta: loadMeta,
    filter: loadFilter,
  } = bindingifyLoad(args)

  const { plugin: renderChunk, meta: renderChunkMeta } =
    bindingifyRenderChunk(args)

  const { plugin: augmentChunkHash, meta: augmentChunkHashMeta } =
    bindingifyAugmentChunkHash(args)

  const { plugin: renderStart, meta: renderStartMeta } =
    bindingifyRenderStart(args)

  const { plugin: renderError, meta: renderErrorMeta } =
    bindingifyRenderError(args)

  const { plugin: generateBundle, meta: generateBundleMeta } =
    bindingifyGenerateBundle(args)

  const { plugin: writeBundle, meta: writeBundleMeta } =
    bindingifyWriteBundle(args)

  const { plugin: closeBundle, meta: closeBundleMeta } =
    bindingifyCloseBundle(args)

  const { plugin: banner, meta: bannerMeta } = bindingifyBanner(args)

  const { plugin: footer, meta: footerMeta } = bindingifyFooter(args)

  const { plugin: intro, meta: introMeta } = bindingifyIntro(args)

  const { plugin: outro, meta: outroMeta } = bindingifyOutro(args)

  const { plugin: watchChange, meta: watchChangeMeta } =
    bindingifyWatchChange(args)

  const { plugin: closeWatcher, meta: closeWatcherMeta } =
    bindingifyCloseWatcher(args)

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
  }
  return wrapHandlers(result)
}

function wrapHandlers(plugin: BindingPluginOptions): BindingPluginOptions {
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
  ] as const) {
    const handler = plugin[hookName] as any
    if (handler) {
      plugin[hookName] = async (...args: any[]) => {
        try {
          return await handler(...args)
        } catch (e: any) {
          return error(
            logPluginError(e, plugin.name, {
              hook: hookName,
              id: hookName === 'transform' ? args[2] : undefined,
            }),
          )
        }
      }
    }
  }
  return plugin
}
