import { z } from 'zod'

export type ModuleSideEffectsOption = z.infer<
  typeof ModuleSideEffectsOptionSchema
>

export const ModuleSideEffectsOptionSchema = z.boolean().or(z.string())

export const NormalizedTreeshakingOptionsSchema = z.strictObject({
  moduleSideEffects: ModuleSideEffectsOptionSchema,
})

export type NormalizedTreeshakingOptions = {
  moduleSideEffects: string
}
