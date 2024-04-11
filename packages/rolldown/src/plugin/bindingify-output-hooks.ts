import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import { RolldownNormalizedInputOptions } from '../options/input-options'
import { NormalizedOutputOptions } from '../options/output-options'
import type { Plugin } from './index'

export function bindingifyRenderStart(
  outputOptions: NormalizedOutputOptions,
  options: RolldownNormalizedInputOptions,
  hook?: Plugin['renderStart'],
): BindingPluginOptions['renderStart'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async () => {
    handler.call(null, outputOptions, options)
  }
}
export function bindingifyGenerateBundle(
  hook?: Plugin['generateBundle'],
): BindingPluginOptions['generateBundle'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (bundle, isWrite) => {
    handler.call(null, bundle, isWrite)
  }
}
export function bindingifyWriteBundle(
  hook?: Plugin['writeBundle'],
): BindingPluginOptions['writeBundle'] {
  if (!hook) {
    return undefined
  }
  const [handler, _optionsIgnoredSofar] = normalizeHook(hook)

  return async (bundle) => {
    handler.call(null, bundle)
  }
}
