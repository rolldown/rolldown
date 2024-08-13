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

export interface NormalizedCliOptions {
  input: InputOptions
  output: OutputOptions
  help: boolean
  config: string
  version: boolean
}

export function normalizeCliOptions(options: CliOptions): NormalizedCliOptions {
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
  const keysOfOutput = outputCliOptionsSchema.keyof()._def.values as string[]
  for (const key of Object.keys(options)) {
    // TODO remove the hard code.
    const keys = key.split('.')
    const [primary, secondary] = keys;
    if (keysOfInput.includes(primary)) {
      // @ts-ignore
      result.input[key] = options[key]
    } else if (keysOfOutput.includes(primary)) {
      // @ts-ignore
      result.output[key] = options[key]
    } else if (!reservedKeys.includes(key)) {
      logger.error(`Unknown option: ${key}`)
    }
  }
  return result
}
