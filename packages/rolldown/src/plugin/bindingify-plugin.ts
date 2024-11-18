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
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'
import { PluginContextData } from './plugin-context-data'
import {
  bindingifyCloseWatcher,
  bindingifyWatchChange,
} from './bindingify-watch-hooks'
import { error, logPluginError } from '../log/logs'

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function bindingifyPlugin(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): BindingPluginOptions {
  const { plugin: buildStart, meta: buildStartMeta } = bindingifyBuildStart(
    plugin,
    options,
    pluginContextData,
  )

  const {
    plugin: resolveId,
    meta: resolveIdMeta,
    filter: resolveIdFilter,
  } = bindingifyResolveId(plugin, options, pluginContextData)

  const { plugin: resolveDynamicImport, meta: resolveDynamicImportMeta } =
    bindingifyResolveDynamicImport(plugin, options, pluginContextData)

  const { plugin: buildEnd, meta: buildEndMeta } = bindingifyBuildEnd(
    plugin,
    options,
    pluginContextData,
  )

  const {
    plugin: transform,
    meta: transformMeta,
    filter: transformFilter,
  } = bindingifyTransform(plugin, options, pluginContextData)

  const { plugin: moduleParsed, meta: moduleParsedMeta } =
    bindingifyModuleParsed(plugin, options, pluginContextData)

  const {
    plugin: load,
    meta: loadMeta,
    filter: loadFilter,
  } = bindingifyLoad(plugin, options, pluginContextData)

  const { plugin: renderChunk, meta: renderChunkMeta } = bindingifyRenderChunk(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )

  const { plugin: augmentChunkHash, meta: augmentChunkHashMeta } =
    bindingifyAugmentChunkHash(plugin, options, pluginContextData)

  const { plugin: renderStart, meta: renderStartMeta } = bindingifyRenderStart(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )

  const { plugin: renderError, meta: renderErrorMeta } = bindingifyRenderError(
    plugin,
    options,
    pluginContextData,
  )

  const { plugin: generateBundle, meta: generateBundleMeta } =
    bindingifyGenerateBundle(plugin, options, outputOptions, pluginContextData)

  const { plugin: writeBundle, meta: writeBundleMeta } = bindingifyWriteBundle(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )

  const { plugin: closeBundle, meta: closeBundleMeta } = bindingifyCloseBundle(
    plugin,
    options,
    pluginContextData,
  )

  const { plugin: banner, meta: bannerMeta } = bindingifyBanner(
    plugin,
    options,
    pluginContextData,
  )
  const { plugin: footer, meta: footerMeta } = bindingifyFooter(
    plugin,
    options,
    pluginContextData,
  )
  const { plugin: intro, meta: introMeta } = bindingifyIntro(
    plugin,
    options,
    pluginContextData,
  )
  const { plugin: outro, meta: outroMeta } = bindingifyOutro(
    plugin,
    options,
    pluginContextData,
  )

  const { plugin: watchChange, meta: watchChangeMeta } = bindingifyWatchChange(
    plugin,
    options,
    pluginContextData,
  )

  const { plugin: closeWatcher, meta: closeWatcherMeta } =
    bindingifyCloseWatcher(plugin, options, pluginContextData)

  const result: BindingPluginOptions = {
    name: plugin.name ?? 'unknown',
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
    // @ts-ignore
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
