import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'

import type { Plugin } from './index'

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
