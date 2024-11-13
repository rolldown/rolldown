import { ModuleSideEffectsRule } from './module-side-effects'

export interface NormalizedTreeshakingOptions {
  moduleSideEffects: boolean | ModuleSideEffectsRule[]
}

export * from './module-side-effects'
