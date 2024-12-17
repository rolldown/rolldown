import { schema as objectSchema } from './schema'
import { alias, type OptionConfig } from './alias'
import {
  camelCaseToKebabCase,
  flattenSchema,
  getSchemaType,
  kebabCaseToCamelCase,
} from './utils'
import { parseArgs } from 'node:util'
import { normalizeCliOptions, type NormalizedCliOptions } from './normalize'
import { logger } from '../logger'
import type { Schema } from './types'

export const flattenedSchema: Record<string, Schema> = flattenSchema(
  objectSchema.properties,
)

export const options: {
  [k: string]: {
    type: 'boolean' | 'string'
    multiple: boolean
    short?: string
    default?: boolean | string | string[]
    hint?: string
    description: string
  }
} = Object.fromEntries(
  Object.entries(flattenedSchema).map(([key, schema]) => {
    const config = Object.getOwnPropertyDescriptor(alias, key)?.value as
      | OptionConfig
      | undefined

    const type = getSchemaType(schema)

    const result = {
      type: type === 'boolean' ? 'boolean' : 'string',
      // We only support comma separated mode right now.
      // multiple: type === 'object' || type === 'array',
      description: schema?.description ?? config?.description ?? '',
      hint: config?.hint,
    } as {
      type: 'boolean' | 'string'
      multiple: boolean
      short?: string
      default?: boolean | string | string[]
      hint?: string
      description: string
    }
    if (config && config?.abbreviation) {
      result.short = config?.abbreviation
    }
    if (config && config.reverse) {
      if (result.description.startsWith('enable')) {
        result.description = result.description.replace('enable', 'disable')
      } else {
        result.description = `disable ${result.description}`
      }
    }
    key = camelCaseToKebabCase(key)
    // add 'no-' prefix for need reverse options
    return [config?.reverse ? `no-${key}` : key, result]
  }),
)

export type ParseArgsOptions = typeof options

export function parseCliArguments(): NormalizedCliOptions {
  const { values, tokens, positionals } = parseArgs({
    options,
    tokens: true,
    allowPositionals: true,
    // We can't use `strict` mode because we should handle the default config file name.
    strict: false,
  })

  tokens
    .filter((token) => token.kind === 'option')
    .forEach((option) => {
      let negative = false
      if (option.name.startsWith('no-')) {
        // stripe `no-` prefix
        const name = kebabCaseToCamelCase(option.name.substring(3))
        if (name in flattenedSchema) {
          // Remove the `no-` in values
          delete values[option.name]
          option.name = name
          negative = true
        }
      }
      delete values[option.name] // Strip the kebab-case options.
      option.name = kebabCaseToCamelCase(option.name)
      let originalType = flattenedSchema[option.name]
      if (!originalType) {
        logger.error(
          `Invalid option: ${option.rawName}. We will ignore this option.`,
        )
        // We will refuse to handle the invalid option, as it may cause unexpected behavior.
        process.exit(1)
      }
      let type = getSchemaType(originalType)
      if (type === 'string' && typeof option.value !== 'string') {
        let opt = option as { name: string }
        // We should use the default value.
        let defaultValue = Object.getOwnPropertyDescriptor(alias, opt.name)
          ?.value as OptionConfig
        Object.defineProperty(values, opt.name, {
          value: defaultValue.default ?? '',
          enumerable: true,
          configurable: true,
          writable: true,
        })
      } else if (type === 'object' && typeof option.value === 'string') {
        const [key, value] = option.value.split(',').map((x) => x.split('='))[0]
        if (!values[option.name]) {
          Object.defineProperty(values, option.name, {
            value: {},
            enumerable: true,
            configurable: true,
            writable: true,
          })
        }
        if (key && value) {
          // TODO support multiple entries.
          Object.defineProperty(values[option.name], key, {
            value,
            enumerable: true,
            configurable: true,
            writable: true,
          })
        }
      } else if (type === 'array' && typeof option.value === 'string') {
        if (!values[option.name]) {
          Object.defineProperty(values, option.name, {
            value: [],
            enumerable: true,
            configurable: true,
            writable: true,
          })
        }
        ;(values[option.name] as string[]).push(option.value)
      } else if (type === 'boolean') {
        Object.defineProperty(values, option.name, {
          value: !negative,
          enumerable: true,
          configurable: true,
          writable: true,
        })
      } else {
        Object.defineProperty(values, option.name, {
          value: option.value ?? '',
          enumerable: true,
          configurable: true,
          writable: true,
        })
      }
    })

  return normalizeCliOptions(values, positionals as string[])
}
