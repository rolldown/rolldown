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
  return {
    name: plugin.name ?? 'unknown',
    buildStart: bindingifyBuildStart(plugin, options, pluginContextData),
    resolveId: bindingifyResolveId(plugin, options, pluginContextData),
    resolveDynamicImport: bindingifyResolveDynamicImport(
      plugin,
      options,
      pluginContextData,
    ),
    buildEnd: bindingifyBuildEnd(plugin, options, pluginContextData),
    transform: bindingifyTransform(plugin, options, pluginContextData),
    moduleParsed: bindingifyModuleParsed(plugin, options, pluginContextData),
    load: bindingifyLoad(plugin, options, pluginContextData),
    renderChunk: bindingifyRenderChunk(
      plugin,
      options,
      outputOptions,
      pluginContextData,
    ),
    augmentChunkHash: bindingifyAugmentChunkHash(
      plugin,
      options,
      pluginContextData,
    ),
    renderStart: bindingifyRenderStart(
      plugin,
      options,
      outputOptions,
      pluginContextData,
    ),
    renderError: bindingifyRenderError(plugin, options, pluginContextData),
    generateBundle: bindingifyGenerateBundle(
      plugin,
      options,
      outputOptions,
      pluginContextData,
    ),
    writeBundle: bindingifyWriteBundle(
      plugin,
      options,
      outputOptions,
      pluginContextData,
    ),
    banner: bindingifyBanner(plugin, options, pluginContextData),
    footer: bindingifyFooter(plugin, options, pluginContextData),
    intro: bindingifyIntro(plugin, options, pluginContextData),
    outro: bindingifyOutro(plugin, options, pluginContextData),
  }
}
