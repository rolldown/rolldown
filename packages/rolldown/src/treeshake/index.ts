import { ModuleSideEffectsRule } from './module-side-effects'

export interface NormalizedTreeshakingOptions {
  moduleSideEffects: boolean | ModuleSideEffectsRule[]
  annotations?: boolean
}

export * from './module-side-effects'
