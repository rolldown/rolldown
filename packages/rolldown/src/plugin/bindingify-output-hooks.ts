import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { Plugin } from './index'
import { transformToOutputBundle } from '../utils/transform-to-rollup-output'
import { PluginContext } from './plugin-context'
import { bindingifySourcemap } from '../types/sourcemap'
import { NormalizedOutputOptions } from '../options/normalized-output-options'

export function bindingifyRenderStart(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingPluginOptions['renderStart'] {
  const hook = plugin.renderStart
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx) => {
    handler.call(
      new PluginContext(options, ctx, plugin),
      outputOptions,
      options,
    )
  }
}

export function bindingifyRenderChunk(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingPluginOptions['renderChunk'] {
  const hook = plugin.renderChunk
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, code, chunk) => {
    const ret = await handler.call(
      new PluginContext(options, ctx, plugin),
      code,
      chunk,
      outputOptions,
    )

    if (ret == null) {
      return
    }

    if (typeof ret === 'string') {
      return { code: ret }
    }

    if (!ret.map) {
      return { code: ret.code }
    }

    return {
      code: ret.code,
      map: bindingifySourcemap(ret.map),
    }
  }
}

export function bindingifyAugmentChunkHash(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['augmentChunkHash'] {
  const hook = plugin.augmentChunkHash
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, chunk) => {
    return await handler.call(new PluginContext(options, ctx, plugin), chunk)
  }
}

export function bindingifyRenderError(
  plugin: Plugin,
  options: NormalizedInputOptions,
): BindingPluginOptions['renderError'] {
  const hook = plugin.renderError
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, err) => {
    handler.call(new PluginContext(options, ctx, plugin), new Error(err))
  }
}

export function bindingifyGenerateBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingPluginOptions['generateBundle'] {
  const hook = plugin.generateBundle
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, bundle, isWrite) => {
    handler.call(
      new PluginContext(options, ctx, plugin),
      outputOptions,
      transformToOutputBundle(bundle),
      isWrite,
    )
  }
}
export function bindingifyWriteBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
): BindingPluginOptions['writeBundle'] {
  const hook = plugin.writeBundle
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, bundle) => {
    handler.call(
      new PluginContext(options, ctx, plugin),
      outputOptions,
      transformToOutputBundle(bundle),
    )
  }
}
