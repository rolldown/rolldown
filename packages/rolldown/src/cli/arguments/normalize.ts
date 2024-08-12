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

export interface NormalizedCliOptions {
  input: InputOptions
  output: OutputOptions
  help: boolean
  config: string
  version: boolean
}

export function normalizeCliOptions(options: CliOptions): NormalizedCliOptions {
  const result = {
    input: {},
    output: {},
    help: options.help ?? false,
    version: options.version ?? false,
    config:
      typeof options.config === 'boolean'
        ? options.config
          ? 'rolldown.config.js'
          : ''
        : (options.config ?? ''),
  } as NormalizedCliOptions
  const keysOfInput = inputCliOptionsSchema.keyof()._def.values as string[]
  const keysOfOutput = outputCliOptionsSchema.keyof()._def.values as string[]
  for (const key of Object.keys(options)) {
    if (keysOfInput.includes(key)) {
      // @ts-ignore
      result.input[key] = options[key]
    } else if (keysOfOutput.includes(key)) {
      // @ts-ignore
      result.output[key] = options[key]
    }
  }
  return result
}
