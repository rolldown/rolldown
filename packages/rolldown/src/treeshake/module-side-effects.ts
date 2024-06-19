import { z } from 'zod'

export const HasModuleSideEffectsSchema = z
  .function()
  .args(z.string(), z.boolean())
  .returns(z.boolean())
export type HasModuleSideEffects = z.infer<typeof HasModuleSideEffectsSchema>

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
