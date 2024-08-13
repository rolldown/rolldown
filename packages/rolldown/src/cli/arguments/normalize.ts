/**
 * @description This file is used for normalize the options.
 * In CLI, the input options and output options are mixed together. We need to tell them apart.
 */
import {
  inputCliOptionsSchema,
  InputOptions,
} from '../../options/input-options'
import {
  outputCliOptionsSchema,
  OutputOptions,
} from '../../options/output-options'
import type { CliOptions } from './schema'
import { logger } from '../utils'
import { ParseArgsOptions } from '.'
import { alias, OptionConfig } from './alias'
import { setNestedProperty } from './utils'

export interface NormalizedCliOptions {
  input: InputOptions
  output: OutputOptions
  help: boolean
  config: string
  version: boolean
}

export function normalizeCliOptions(
  options: CliOptions,
  positionals: string[],
  args: ParseArgsOptions,
): NormalizedCliOptions {
  const result = {
    input: {} as InputOptions,
    output: {} as OutputOptions,
    help: options.help ?? false,
    version: options.version ?? false,
    config:
      typeof options.config === 'boolean'
        ? options.config
          ? 'rolldown.config.js'
          : ''
        : (options.config ?? ''),
  } as NormalizedCliOptions
  const reservedKeys = ['help', 'version', 'config']
  const keysOfInput = inputCliOptionsSchema.keyof()._def.values as string[]
  // Because input is the positional args, we shouldn't include it in the input schema.
  const keysOfOutput = outputCliOptionsSchema.keyof()._def.values as string[]
  for (let [key, value] of Object.entries(options)) {
    const keys = key.split('.')
    const [primary] = keys
    if (!args[key]) continue // Ignore the unknown options
    if (args[key].type === 'string' && value === true) {
      const config: OptionConfig = Object.getOwnPropertyDescriptor(
        alias,
        key,
      )?.value
      if (config.default) {
        value = config.default
      } else {
        logger.error(
          `Invalid value for option: ${key}. You should pass a string value.`,
        )
        process.exit(1)
      }
    }
    if (keysOfInput.includes(primary)) {
      setNestedProperty(result.input, key, value)
    } else if (keysOfOutput.includes(primary)) {
      setNestedProperty(result.output, key, value)
    } else if (!reservedKeys.includes(key)) {
      logger.error(`Unknown option: ${key}`)
    }
  }
  if (!result.config && positionals.length > 0) {
    result.input.input = positionals
  }
  return result
}
