import { schema as objectSchema } from './schema'
import { alias, type OptionConfig } from './alias'
import { flattenSchema, getSchemaType } from './utils'
import { parseArgs } from 'node:util'
import { normalizeCliOptions } from './normalize'
import { logger } from '../utils'

export const flattenedSchema = flattenSchema(objectSchema.properties)

export const options = Object.fromEntries(
  Object.entries(flattenedSchema).map(([key, schema]) => {
    const config = Object.getOwnPropertyDescriptor(alias, key)?.value as
      | OptionConfig
      | undefined

    const type = getSchemaType(schema)

    const result = {
      type: type === 'boolean' ? 'boolean' : 'string',
      // We only support comma separated mode right now.
      // multiple: type === 'object' || type === 'array',
      description: config?.description ?? '',
      hint: config?.hint,
    } as {
      type: 'boolean' | 'string'
      multiple: boolean
      short?: string
      default?: boolean | string | string[]
      hint?: string
      description?: string
    }
    if (config && config?.abbreviation) {
      result.short = config?.abbreviation
    }
    return [key, result]
  }),
)

export type ParseArgsOptions = typeof options

export function parseCliArguments() {
  const { values, tokens, positionals } = parseArgs({
    options,
    tokens: true,
    allowPositionals: true,
    allowNegative: true,
    // We can't use `strict` mode because we should handle the default config file name.
    strict: false,
  })

  tokens
    .filter((token) => token.kind === 'option')
    .forEach((option) => {
      let originalType = flattenedSchema[option.name]
      if (!originalType) {
        logger.warn(
          `Invalid option: ${option.rawName}. We will ignore this option.`,
        )
        return
      }
      let type = getSchemaType(originalType)
      if (type === 'object' && typeof option.value === 'string') {
        const mappings = option.value?.split(',').map((x) => x.split('='))
        if (mappings) {
          // TODO support multiple entries.
          values[option.name] = Object.fromEntries(mappings)
        }
      } else if (type === 'array' && typeof option.value === 'string') {
        Object.defineProperty(values, option.name, {
          value: option.value?.split(',') ?? [],
          enumerable: true,
          configurable: true,
          writable: true,
        })
      }
    })

  return normalizeCliOptions(values, positionals as string[], options)
}
