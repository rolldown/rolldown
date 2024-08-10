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
  const [buildStart, buildStartMeta] = bindingifyBuildStart(
    plugin,
    options,
    pluginContextData,
  )
  const [resolveId, resolveIdMeta] = bindingifyResolveId(
    plugin,
    options,
    pluginContextData,
  )
  const [resolveDynamicImport, resolveDynamicImportMeta] =
    bindingifyResolveDynamicImport(plugin, options, pluginContextData)
  const [buildEnd, buildEndMeta] = bindingifyBuildEnd(
    plugin,
    options,
    pluginContextData,
  )
  const [transform, transformMeta] = bindingifyTransform(
    plugin,
    options,
    pluginContextData,
  )
  const [moduleParsed, moduleParsedMeta] = bindingifyModuleParsed(
    plugin,
    options,
    pluginContextData,
  )
  const [load, loadMeta] = bindingifyLoad(plugin, options, pluginContextData)
  const [renderChunk, renderChunkMeta] = bindingifyRenderChunk(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )
  const [augmentChunkHash, augmentChunkHashMeta] = bindingifyAugmentChunkHash(
    plugin,
    options,
    pluginContextData,
  )
  const [renderStart, renderStartMeta] = bindingifyRenderStart(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )
  const [renderError, renderErrorMeta] = bindingifyRenderError(
    plugin,
    options,
    pluginContextData,
  )
  const [generateBundle, generateBundleMeta] = bindingifyGenerateBundle(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )
  const [writeBundle, writeBundleMeta] = bindingifyWriteBundle(
    plugin,
    options,
    outputOptions,
    pluginContextData,
  )
  const [banner, bannerMeta] = bindingifyBanner(
    plugin,
    options,
    pluginContextData,
  )
  const [footer, footerMeta] = bindingifyFooter(
    plugin,
    options,
    pluginContextData,
  )
  const [intro, introMeta] = bindingifyIntro(plugin, options, pluginContextData)
  const [outro, outroMeta] = bindingifyOutro(plugin, options, pluginContextData)

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
