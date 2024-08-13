import process from 'node:process'
import { parseArgs } from 'node:util'
import { mapValues, omit } from 'remeda'
import { bundle } from './commands/bundle'
import { showHelp } from './commands/help'
import { DEFAULT_CONFIG_FILENAME } from './constants'
import { CLI_OPTIONS } from './options'
import { logger } from './utils'

async function main() {
  const { values } = parseArgs({
    options: mapValues(CLI_OPTIONS, omit(['description'])),
    // We need to support both `rolldown -c` and `rolldown -c rolldown.config.js`,
    // the value of the option could be either a boolean or a string in this case,
    // so `strict` needs to be set to `false`
    strict: false,
  })

  if (values.config) {
    // If config is specified, we will ignore other arguments and bundle with the specified config
    await bundle(
      typeof values.config === 'string'
        ? values.config
        : DEFAULT_CONFIG_FILENAME,
    )
    process.exit(0)
  }

  // TODO: accept other arguments

  showHelp()
}

main().catch(logger.error)
