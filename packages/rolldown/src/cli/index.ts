import process from 'node:process'
import { bundleWithCliOptions, bundleWithConfig } from './commands/bundle'
import { logger } from './utils'
import { parseCliArguments } from './arguments'
import { showHelp } from './commands/help'

async function main() {
  const cliOptions = parseCliArguments()

  if (cliOptions.config) {
    await bundleWithConfig(cliOptions.config, cliOptions)
    process.exit(0)
  }

  if ('input' in cliOptions.input) {
    // If input is specified, we will bundle with the input options
    await bundleWithCliOptions(cliOptions)
    process.exit(0)
  }

  showHelp()
}

main().catch(logger.error)
