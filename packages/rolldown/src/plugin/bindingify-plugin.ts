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
  const { pluginOption: buildStart, meta: buildStartMeta } =
    bindingifyBuildStart(plugin, options, pluginContextData)

  const { pluginOption: resolveId, meta: resolveIdMeta } = bindingifyResolveId(
    plugin,
    options,
    pluginContextData,
  )

  const { pluginOption: resolveDynamicImport, meta: resolveDynamicImportMeta } =
    bindingifyResolveDynamicImport(plugin, options, pluginContextData)

  const { pluginOption: buildEnd, meta: buildEndMeta } = bindingifyBuildEnd(
    plugin,
    options,
    pluginContextData,
  )

  const { pluginOption: transform, meta: transformMeta } = bindingifyTransform(
    plugin,
    options,
    pluginContextData,
  )

  const { pluginOption: moduleParsed, meta: moduleParsedMeta } =
    bindingifyModuleParsed(plugin, options, pluginContextData)

  const { pluginOption: load, meta: loadMeta } = bindingifyLoad(
    plugin,
    options,
    pluginContextData,
  )

  const { pluginOption: renderChunk, meta: renderChunkMeta } =
    bindingifyRenderChunk(plugin, options, outputOptions, pluginContextData)

  const { pluginOption: augmentChunkHash, meta: augmentChunkHashMeta } =
    bindingifyAugmentChunkHash(plugin, options, pluginContextData)

  const { pluginOption: renderStart, meta: renderStartMeta } =
    bindingifyRenderStart(plugin, options, outputOptions, pluginContextData)

  const { pluginOption: renderError, meta: renderErrorMeta } =
    bindingifyRenderError(plugin, options, pluginContextData)

  const { pluginOption: generateBundle, meta: generateBundleMeta } =
    bindingifyGenerateBundle(plugin, options, outputOptions, pluginContextData)

  const { pluginOption: writeBundle, meta: writeBundleMeta } =
    bindingifyWriteBundle(plugin, options, outputOptions, pluginContextData)

  const { pluginOption: banner, meta: bannerMeta } = bindingifyBanner(
    plugin,
    options,
    pluginContextData,
  )
  const { pluginOption: footer, meta: footerMeta } = bindingifyFooter(
    plugin,
    options,
    pluginContextData,
  )
  const { pluginOption: intro, meta: introMeta } = bindingifyIntro(
    plugin,
    options,
    pluginContextData,
  )
  const { pluginOption: outro, meta: outroMeta } = bindingifyOutro(
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
    resolveDynamicImport,
    resolveDynamicImportMeta,
    buildEnd,
    buildEndMeta,
    transform,
    transformMeta,
    moduleParsed,
    moduleParsedMeta,
    load,
    loadMeta,
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
