import {
  ModuleSideEffectsOptionSchema,
  NormalizedTreeshakingOptionsSchema,
} from './module-side-effects'

export const TreeshakingOptionsSchema =
  NormalizedTreeshakingOptionsSchema.extend({
    moduleSideEffects: ModuleSideEffectsOptionSchema.optional(),
  })

export interface TreeshakingOptions {
  moduleSideEffects?: boolean | RegExp
}

export * from './module-side-effects'
