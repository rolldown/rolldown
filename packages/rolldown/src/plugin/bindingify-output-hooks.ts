import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import { NormalizedOutputOptions } from '../options/output-options'
import type { Plugin } from './index'
import { transformToOutputBundle } from '../utils/transform-to-rollup-output'
import { transformPluginContext } from './plugin-context'

export function bindingifyRenderStart(
  outputOptions: NormalizedOutputOptions,
  options: NormalizedInputOptions,
  hook?: Plugin['renderStart'],
): BindingPluginOptions['renderStart'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx) => {
    handler.call(transformPluginContext(ctx), outputOptions, options)
  }
}

export function bindingifyRenderChunk(
  outputOptions: NormalizedOutputOptions,
  hook?: Plugin['renderChunk'],
): BindingPluginOptions['renderChunk'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, code, chunk) => {
    const ret = await handler.call(
      transformPluginContext(ctx),
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
      map: typeof ret.map === 'object' ? JSON.stringify(ret.map) : ret.map,
    }
  }
}

export function bindingifyRenderError(
  hook?: Plugin['renderError'],
): BindingPluginOptions['renderError'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, err) => {
    handler.call(transformPluginContext(ctx), new Error(err))
  }
}

export function bindingifyGenerateBundle(
  outputOptions: NormalizedOutputOptions,
  hook?: Plugin['generateBundle'],
): BindingPluginOptions['generateBundle'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, bundle, isWrite) => {
    handler.call(
      transformPluginContext(ctx),
      outputOptions,
      transformToOutputBundle(bundle),
      isWrite,
    )
  }
}
export function bindingifyWriteBundle(
  outputOptions: NormalizedOutputOptions,
  hook?: Plugin['writeBundle'],
): BindingPluginOptions['writeBundle'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (ctx, bundle) => {
    handler.call(
      transformPluginContext(ctx),
      outputOptions,
      transformToOutputBundle(bundle),
    )
  }
}
