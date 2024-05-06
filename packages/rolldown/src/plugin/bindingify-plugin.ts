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
} from './bindingify-output-hooks'

import type { Plugin } from './index'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { NormalizedOutputOptions } from '../options/output-options'

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function bindingifyPlugin(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingPluginOptions {
  return {
    name: plugin.name ?? 'unknown',
    buildStart: bindingifyBuildStart(options, plugin.buildStart),
    resolveId: bindingifyResolveId(plugin.resolveId),
    resolveDynamicImport: bindingifyResolveDynamicImport(
      plugin.resolveDynamicImport,
    ),
    buildEnd: bindingifyBuildEnd(plugin.buildEnd),
    transform: bindingifyTransform(plugin.transform),
    moduleParsed: bindingifyModuleParsed(plugin.moduleParsed),
    load: bindingifyLoad(plugin.load),
    renderChunk: bindingifyRenderChunk(outputOptions, plugin.renderChunk),
    renderStart: bindingifyRenderStart(
      outputOptions,
      options,
      plugin.renderStart,
    ),
    renderError: bindingifyRenderError(plugin.renderError),
    generateBundle: bindingifyGenerateBundle(
      outputOptions,
      plugin.generateBundle,
    ),
    writeBundle: bindingifyWriteBundle(outputOptions, plugin.writeBundle),
  }
}
