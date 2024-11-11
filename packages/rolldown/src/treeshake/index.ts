import {
  ModuleSideEffectsOptionSchema,
  NormalizedTreeshakingOptionsSchema,
} from './module-side-effects'

export interface TreeshakingOptions {
  moduleSideEffects?: boolean | ModuleSideEffectsRule[]
}

export type ModuleSideEffectsRule = {
  test?: RegExp,
  external?: boolean,
  sideEffects: boolean
}

export * from './module-side-effects'
