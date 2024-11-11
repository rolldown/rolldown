import { z } from 'zod'

export type ModuleSideEffectsOption = z.infer<
  typeof ModuleSideEffectsOptionSchema
>

export const ModuleSideEffectsRuleSchema = z
  .object({
    test: z.instanceof(RegExp).optional(),
    external: z.boolean().optional(),
    sideEffects: z.boolean(),
  })
  .refine((data) => {
    data.test || data.external
  }, 'Either `test` or `external` should be set.')

export const ModuleSideEffectsOptionSchema = z
  .boolean()
  .or(z.array(ModuleSideEffectsRuleSchema))

export const NormalizedTreeshakingOptionsSchema = z.strictObject({
  moduleSideEffects: ModuleSideEffectsOptionSchema,
})

export type NormalizedTreeshakingOptions = z.infer<
  typeof NormalizedTreeshakingOptionsSchema
>
