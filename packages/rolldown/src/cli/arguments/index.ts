import { schema as objectSchema } from './schema'
import { alias, type OptionConfig } from './alias'
import { flattenSchema, getSchemaType } from './utils'
import { parseArgs } from 'node:util'
import { normalizeCliOptions } from './normalize'

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
    strict: false,
  })

  tokens
    .filter((token) => token.kind === 'option')
    .forEach((option) => {
      let originalType = flattenedSchema[option.name]
      let type = getSchemaType(originalType)
      if (option.name.startsWith('no-')) {
        option.name = option.name.slice(3)
        values[option.name] = false
      }
      if (type === 'object') {
        const mappings = option.value?.split(',').map((x) => x.split('='))
        if (mappings) {
          values[option.name] = Object.fromEntries(mappings)
        }
      } else if (type === 'array') {
        ;(values[option.name] as string[]) = option.value?.split(',') ?? []
      }
    })

  if (!values.config && positionals.length !== 0) {
    ;(values.input as string[]) = positionals as string[]
  }

  return normalizeCliOptions(values)
}
