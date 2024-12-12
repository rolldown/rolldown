import process from 'node:process'
import { bundleWithCliOptions, bundleWithConfig } from './commands/bundle'
import { logger } from './logger'
import { parseCliArguments } from './arguments'
import { showHelp } from './commands/help'
import { version } from '../../package.json'

async function main() {
  const cliOptions = parseCliArguments()

  if (cliOptions.config) {
    await bundleWithConfig(cliOptions.config, cliOptions)
    return
  }

  if ('input' in cliOptions.input) {
    // If input is specified, we will bundle with the input options
    await bundleWithCliOptions(cliOptions)
    return
  }

  if (cliOptions.version) {
    logger.log(`rolldown v${version}`)
    return
  }

  showHelp()
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
