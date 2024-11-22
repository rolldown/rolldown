import type { InputOptions } from '../types/input-options'
import { NormalizedTreeshakingOptions } from '../treeshake'

export function normalizeTreeshakeOptions(
  config: InputOptions['treeshake'],
): NormalizedTreeshakingOptions | undefined {
  if (config === false) {
    return undefined
  }
  if (config === true || config === undefined) {
    return {
      moduleSideEffects: true,
      annotations: true,
    }
  }
  let normalizedConfig: NormalizedTreeshakingOptions = {
    moduleSideEffects: true,
  }
  if (config.moduleSideEffects === undefined) {
    normalizedConfig.moduleSideEffects = true
  } else if (config.moduleSideEffects === 'no-external') {
    normalizedConfig.moduleSideEffects = [
      { external: true, sideEffects: false },
      { external: false, sideEffects: true },
    ]
  } else {
    normalizedConfig.moduleSideEffects = config.moduleSideEffects
  }

  normalizedConfig.annotations = config.annotations ?? true
  return normalizedConfig
}
