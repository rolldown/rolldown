import { schema as objectSchema } from './schema'
import { alias, type OptionConfig } from './alias'
import { flattenSchema, getSchemaType } from './utils'
import { parseArgs } from 'node:util'
import { normalizeCliOptions } from './normalize'
import { logger } from '../utils'

export const flattenedSchema = flattenSchema(objectSchema.properties)

const options = Object.fromEntries(
  Object.entries(flattenedSchema).map(([key, schema]) => {
    const config = Object.getOwnPropertyDescriptor(alias, key)?.value as
      | OptionConfig
      | undefined

    const type = getSchemaType(schema)

    const result = {
      type: type === 'boolean' ? 'boolean' : 'string',
      multiple: type === 'object' || type === 'array',
    } as {
      type: 'boolean' | 'string'
      multiple: boolean
      short?: string
      default?: boolean | string | string[]
    }
    if (config && config?.abbreviation) {
      result.short = config?.abbreviation
    }
    if (config && config?.default) {
      result.default = config.default
    }
    return [key, result]
  }),
)

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
      let type = getSchemaType(originalType)
      if (type !== 'boolean' && typeof option.value !== 'string') {
        logger.error('Invalid value for option: ' + option.name)
      }
      if (type === 'object') {
        const mappings = option.value?.split(',').map((x) => x.split('='))
        if (mappings) {
          // TODO support multiple entries.
          values[option.name] = Object.fromEntries(mappings)
        }
      } else if (type === 'array') {
        // TODO support multiple entries.
        ;(values[option.name] as string[]) = option.value?.split(',') ?? []
      }
    })

  if (!values.config && positionals.length !== 0) {
    ;(values.input as string[]) = positionals as string[]
  }

  return normalizeCliOptions(values)
}
