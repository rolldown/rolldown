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
    return data.test !== undefined || data.external !== undefined
  }, 'Either `test` or `external` should be set.')

export const ModuleSideEffectsOptionSchema = z
  .boolean()
  .or(z.array(ModuleSideEffectsRuleSchema))
  .or(
    z.function().args(z.string(), z.boolean()).returns(z.boolean().optional()),
  )
  .or(z.literal('no-external'))

export const TreeshakingOptionsSchema = z
  .object({
    moduleSideEffects: ModuleSideEffectsOptionSchema.optional(),
    annotations: z.boolean().optional(),
  })
  .passthrough()

  .or(z.boolean())

export type TreeshakingOptions = z.infer<typeof TreeshakingOptionsSchema>
