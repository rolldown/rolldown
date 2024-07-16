import type { InputOptions } from '../options/input-options'
import { NormalizedTreeshakingOptions } from '../treeshake'
import { isRegExp } from 'node:util/types'

export function normalizeTreeshakeOptions(
  config: InputOptions['treeshake'],
): NormalizedTreeshakingOptions | undefined {
  if (config === false) {
    return undefined
  }
  if (config === true || config === undefined) {
    return {
      moduleSideEffects: 'true',
    }
  }
  let normalizedConfig: NormalizedTreeshakingOptions = {
    moduleSideEffects: '',
  }
  if (config.moduleSideEffects === undefined) {
    normalizedConfig.moduleSideEffects = 'true'
  } else if (isRegExp(config.moduleSideEffects)) {
    normalizedConfig.moduleSideEffects = config.moduleSideEffects.source
  } else {
    normalizedConfig.moduleSideEffects = config.moduleSideEffects.toString()
  }
  return normalizedConfig
}
