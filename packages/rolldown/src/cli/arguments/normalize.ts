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
import { CliOptions, cliOptionsSchema } from './schema'
import { logger } from '../utils'
import { setNestedProperty } from './utils'

export interface NormalizedCliOptions {
  input: InputOptions
  output: OutputOptions
  help: boolean
  config: string
  version: boolean
}

export function normalizeCliOptions(
  cliOptions: CliOptions,
  positionals: string[],
): NormalizedCliOptions {
  const parsed = cliOptionsSchema.safeParse(cliOptions)
  const options = parsed.data ?? {}
  if (!parsed.success) {
    parsed.error.errors.forEach((error) => {
      logger.error(
        `Invalid value for option ${error.path.join(', ')}. You can use \`rolldown -h\` to see the help.`,
      )
    })
    process.exit(1)
  }
  const result = {
    input: {} as InputOptions,
    output: {} as OutputOptions,
    help: options.help ?? false,
    version: options.version ?? false,
  } as NormalizedCliOptions
  if (typeof options.config === 'string') {
    result.config = options.config ? options.config : 'rolldown.config.js'
  }
  const reservedKeys = ['help', 'version', 'config']
  const keysOfInput = inputCliOptionsSchema.keyof()._def.values as string[]
  // Because input is the positional args, we shouldn't include it in the input schema.
  const keysOfOutput = outputCliOptionsSchema.keyof()._def.values as string[]
  for (let [key, value] of Object.entries(options)) {
    const keys = key.split('.')
    const [primary] = keys
    if (keysOfInput.includes(primary)) {
      setNestedProperty(result.input, key, value)
    } else if (keysOfOutput.includes(primary)) {
      setNestedProperty(result.output, key, value)
    } else if (!reservedKeys.includes(key)) {
      logger.error(`Unknown option: ${key}`)
      process.exit(1)
    }
  }
  if (!result.config && positionals.length > 0) {
    result.input.input = positionals
  }
  return result
}
