import { BindingTransformPluginConfig } from '../binding'
import { normalizedStringOrRegex } from './utils'

type TransformPattern = string | RegExp | (RegExp | string)[]
// A temp config type for giving better user experience
export type TransformPluginConfig = Omit<
  BindingTransformPluginConfig,
  'include' | 'exclude'
> & {
  include?: TransformPattern
  exclude?: TransformPattern
}

export function normalizeEcmaTransformPluginConfig(
  config?: TransformPluginConfig,
): BindingTransformPluginConfig | undefined {
  if (!config) {
    return undefined
  }
  let normalizedConfig: BindingTransformPluginConfig = {
    jsxInject: config?.jsxInject,
    exclude: normalizedStringOrRegex(config.exclude),
    include: normalizedStringOrRegex(config.include),
  }

  return normalizedConfig
}
