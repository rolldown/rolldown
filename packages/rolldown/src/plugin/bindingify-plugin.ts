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
} from './bindingify-output-hooks'

import type { Plugin } from './index'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { NormalizedOutputOptions } from '../options/normalized-output-options'

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function bindingifyPlugin(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingPluginOptions {
  return {
    name: plugin.name ?? 'unknown',
    buildStart: bindingifyBuildStart(plugin, options),
    resolveId: bindingifyResolveId(plugin, options),
    resolveDynamicImport: bindingifyResolveDynamicImport(plugin, options),
    buildEnd: bindingifyBuildEnd(plugin, options),
    transform: bindingifyTransform(plugin, options),
    moduleParsed: bindingifyModuleParsed(plugin, options),
    load: bindingifyLoad(plugin, options),
    renderChunk: bindingifyRenderChunk(plugin, options, outputOptions),
    augmentChunkHash: bindingifyAugmentChunkHash(plugin, options),
    renderStart: bindingifyRenderStart(plugin, options, outputOptions),
    renderError: bindingifyRenderError(plugin, options),
    generateBundle: bindingifyGenerateBundle(plugin, options, outputOptions),
    writeBundle: bindingifyWriteBundle(plugin, options, outputOptions),
  }
}
