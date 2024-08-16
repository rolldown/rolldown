import { isRegExp } from 'node:util/types'
import { BindingTransformPluginConfig } from '../binding'

type TransformPattern = string | RegExp | RegExp[] | string[]
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
  }

  if (config?.exclude) {
    let exclude: (string | RegExp)[] = []
    if (isRegExp(config.exclude)) {
      exclude = [config.exclude]
    } else if (typeof config.exclude === 'string') {
      exclude = [config.exclude]
    } else {
      exclude = config.exclude
    }
    normalizedConfig.exclude = []
    for (let item of exclude) {
      if (isRegExp(item)) {
        normalizedConfig.exclude.push({ value: item.source, flag: item.flags })
      } else {
        normalizedConfig.exclude.push({ value: item })
      }
    }
  }

  if (config?.include) {
    let include: (string | RegExp)[] = []
    if (isRegExp(config.include)) {
      include = [config.include]
    } else if (typeof config.include === 'string') {
      include = [config.include]
    } else {
      include = config.include
    }
    normalizedConfig.include = []
    for (let item of include) {
      if (isRegExp(item)) {
        normalizedConfig.include.push({ value: item.source, flag: item.flags })
      } else {
        normalizedConfig.include.push({ value: item })
      }
    }
  }
  return normalizedConfig
}
