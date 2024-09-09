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

  return {
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
  }
}
