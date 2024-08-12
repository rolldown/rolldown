import { zodToJsonSchema } from 'zod-to-json-schema'
import { inputCliOptionsSchema } from '../../options/input-options'
import { outputCliOptionsSchema } from '../../options/output-options'
import type { ObjectSchema } from './types'
import { z } from 'zod'

export const cliOptionsSchema = z.strictObject({
  config: z.boolean().optional(),
  help: z.boolean().optional()
}).merge(inputCliOptionsSchema).merge(outputCliOptionsSchema)

export type CliOptions = z.infer<typeof cliOptionsSchema> & {
  help?: boolean | string
  config?: boolean | string
}

export const schema = zodToJsonSchema(cliOptionsSchema) as unknown as ObjectSchema
