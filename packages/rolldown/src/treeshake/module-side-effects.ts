import type Z from 'zod'
import { z } from 'zod'

export interface ModuleSideEffectsRule {
  test?: RegExp
  external?: boolean
  sideEffects: boolean
}

export const ModuleSideEffectsRuleSchema: Z.ZodType<ModuleSideEffectsRule> = z
  .object({
    test: z.instanceof(RegExp).optional(),
    external: z.boolean().optional(),
    sideEffects: z.boolean(),
  })
  .refine((data) => {
    return data.test !== undefined || data.external !== undefined
  }, 'Either `test` or `external` should be set.')

export type ModuleSideEffectsOption =
  | boolean
  | ModuleSideEffectsRule[]
  | ((id: string, isResolved: boolean) => boolean | undefined)
  | 'no-external'

export const ModuleSideEffectsOptionSchema: Z.ZodType<ModuleSideEffectsOption> =
  z
    .boolean()
    .or(z.array(ModuleSideEffectsRuleSchema))
    .or(
      z
        .function()
        .args(z.string(), z.boolean())
        .returns(z.boolean().optional()),
    )
    .or(z.literal('no-external'))

export type TreeshakingOptions =
  | {
      moduleSideEffects?: ModuleSideEffectsOption
      annotations?: boolean
    }
  | boolean

export const TreeshakingOptionsSchema: Z.ZodType<TreeshakingOptions> = z
  .object({
    moduleSideEffects: ModuleSideEffectsOptionSchema.optional(),
    annotations: z.boolean().optional(),
  })
  .passthrough()
  .or(z.boolean())
