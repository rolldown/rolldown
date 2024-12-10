import { zodToJsonSchema } from 'zod-to-json-schema'
import { inputCliOptionsSchema } from '../../options/input-options-schema'
import { InputCliOptions } from '../../options/input-options'
import { OutputCliOptions } from '../../options/output-options'
import { outputCliOptionsSchema } from '../../options/output-options-schema'
import type { ObjectSchema } from './types'
import type Z from 'zod'
import { z } from 'zod'

export interface CliOptions extends InputCliOptions, OutputCliOptions {
  config?: string | boolean
  help?: boolean
  version?: boolean
  watch?: boolean
}

export const cliOptionsSchema: Z.ZodType<CliOptions> = z
  .strictObject({
    config: z
      .string()
      .or(z.boolean())
      .describe('Path to the config file (default: `rolldown.config.js`)')
      .optional(),
    help: z.boolean().describe('Show help').optional(),
    version: z.boolean().describe('Show version number').optional(),
    watch: z
      .boolean()
      .describe('Watch files in bundle and rebuild on changes')
      .optional(),
  })
  .merge(inputCliOptionsSchema as Z.AnyZodObject)
  .merge(outputCliOptionsSchema as Z.AnyZodObject) as any // We already explicitly defined the type of `cliOptionsSchema` as `Z.ZodType<CliOptions>`, so we can safely cast it to `any` here.

export const schema = zodToJsonSchema(
  cliOptionsSchema,
) as unknown as ObjectSchema
