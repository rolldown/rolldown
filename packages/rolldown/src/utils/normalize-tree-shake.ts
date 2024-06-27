import type { InputOptions } from '../options/input-options'
import { NormalizedTreeshakingOptions } from '../../src/treeshake'
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
  if (config.moduleSideEffects === undefined) {
    config.moduleSideEffects = 'true'
  } else if (isRegExp(config.moduleSideEffects)) {
    config.moduleSideEffects = config.moduleSideEffects.source
  } else {
    config.moduleSideEffects = config.moduleSideEffects.toString()
  }
  return config as NormalizedTreeshakingOptions
}
