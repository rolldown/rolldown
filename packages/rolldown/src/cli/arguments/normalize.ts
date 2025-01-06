/**
 * @description This file is used for normalize the options.
 * In CLI, the input options and output options are mixed together. We need to tell them apart.
 */
import { logger } from '../logger'
import { setNestedProperty } from './utils'
import {
  getInputCliKeys,
  getOutputCliKeys,
  validateCliOptions,
} from '../../utils/validator'
import type { CliOptions } from './alias'
import type { InputOptions } from '../../options/input-options'
import type { OutputOptions } from '../../options/output-options'

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
  const [data, errors] = validateCliOptions<CliOptions>(cliOptions)
  if (errors?.length) {
    errors.forEach((error) => {
      logger.error(
        `Invalid value for option ${error}. You can use \`rolldown -h\` to see the help.`,
      )
    })
    process.exit(1)
  }

  const options = data ?? {}
  const result = {
    input: {} as InputOptions,
    output: {} as OutputOptions,
    help: options.help ?? false,
    version: options.version ?? false,
    watch: options.watch ?? false,
  } as NormalizedCliOptions

  if (typeof options.config === 'string') {
    result.config = options.config
  }

  const keysOfInput = getInputCliKeys()
  const keysOfOutput = getOutputCliKeys()
  const reservedKeys = ['help', 'version', 'config', 'watch']

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
