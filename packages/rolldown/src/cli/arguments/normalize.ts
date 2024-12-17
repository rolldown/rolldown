/**
 * @description This file is used for normalize the options.
 * In CLI, the input options and output options are mixed together. We need to tell them apart.
 */
import { logger } from '../logger'
import { setNestedProperty } from './utils'
import { CliOptions, cliOptionsSchema } from './schema'
import { inputCliOptionsSchema } from '../../options/input-options-schema'
import { outputCliOptionsSchema } from '../../options/output-options-schema'
import type { InputOptions } from '../../options/input-options'
import type { OutputOptions } from '../../options/output-options'
import type Z from 'zod'

export interface NormalizedCliOptions {
  input: InputOptions
  output: OutputOptions
  help: boolean
  config: string
  version: boolean
  watch: boolean
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
    watch: options.watch ?? false,
  } as NormalizedCliOptions
  if (typeof options.config === 'string') {
    result.config = options.config ? options.config : 'rolldown.config.js'
  }
  const reservedKeys = ['help', 'version', 'config', 'watch']
  const keysOfInput = (inputCliOptionsSchema as Z.AnyZodObject).keyof()._def
    .values as string[]
  // Because input is the positional args, we shouldn't include it in the input schema.
  const keysOfOutput = (outputCliOptionsSchema as Z.AnyZodObject).keyof()._def
    .values as string[]
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
