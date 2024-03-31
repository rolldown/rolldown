import type { BindingPluginOptions } from '../binding'
import {
  bindingifyBuildEnd,
  bindingifyBuildStart,
  bindingifyLoad,
  bindingifyRenderChunk,
  bindingifyResolveId,
  bindingifyTransform,
} from './bindingify-build-hooks'

import {
  bindingifyGenerateBundle,
  bindingifyWriteBundle,
} from './bindingify-output-hooks'

import type { Plugin } from './index'
import { RolldownNormalizedInputOptions } from '../options/input-options'

// Note: because napi not catch error, so we need to catch error and print error to debugger in adapter.
export function bindingifyPlugin(
  plugin: Plugin,
  options: RolldownNormalizedInputOptions,
): BindingPluginOptions {
  return {
    name: plugin.name ?? 'unknown',
    buildStart: bindingifyBuildStart(options, plugin.buildStart),
    resolveId: bindingifyResolveId(plugin.resolveId),
    buildEnd: bindingifyBuildEnd(plugin.buildEnd),
    transform: bindingifyTransform(plugin.transform),
    load: bindingifyLoad(plugin.load),
    renderChunk: bindingifyRenderChunk(plugin.renderChunk),
    generateBundle: bindingifyGenerateBundle(plugin.generateBundle),
    writeBundle: bindingifyWriteBundle(plugin.writeBundle),
  }
}
