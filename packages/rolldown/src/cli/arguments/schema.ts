import { zodToJsonSchema } from 'zod-to-json-schema'
import { inputCliOptionsSchema } from '../../options/input-options'
import { outputCliOptionsSchema } from '../../options/output-options'
import type { ObjectSchema } from './types'
import { z } from 'zod'

export const cliOptionsSchema = inputCliOptionsSchema.merge(outputCliOptionsSchema)

export type CliOptions = z.infer<typeof cliOptionsSchema>

export const schema = zodToJsonSchema(cliOptionsSchema) as unknown as ObjectSchema
