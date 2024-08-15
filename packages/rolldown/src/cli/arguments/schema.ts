import { zodToJsonSchema } from 'zod-to-json-schema'
import { inputCliOptionsSchema } from '../../options/input-options'
import { outputCliOptionsSchema } from '../../options/output-options'
import type { ObjectSchema } from './types'
import { z } from 'zod'

export const cliOptionsSchema = z
  .strictObject({
    config: z
      .string()
      .or(z.boolean())
      .describe('Path to the config file (default: `rollup.config.js`)')
      .optional(),
    help: z.boolean().describe('Show help').optional(),
    version: z.boolean().describe('Show version number').optional(),
  })
  .merge(inputCliOptionsSchema)
  .merge(outputCliOptionsSchema)

export type CliOptions = z.infer<typeof cliOptionsSchema>

export const schema = zodToJsonSchema(
  cliOptionsSchema,
) as unknown as ObjectSchema
