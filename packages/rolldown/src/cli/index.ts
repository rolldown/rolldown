import process from 'node:process'
import parseArgs from 'mri'
import { defineCommand, runMain, showUsage } from 'citty'
import { bundle } from './commands/bundle'
import {
  version,
  description,
} from '../../package.json' assert { type: 'json' }
import { DEFAULT_CONFIG_FILENAME } from './constants'

interface ParsedArgs {
  config?: string | true
  c?: string | true
  // `citty` intercept the help option, so we don't need to deal with it
  // help?: boolean
  // h?: boolean
}

const main = defineCommand({
  meta: {
    name: 'rolldown',
    version,
    description,
  },
  args: {
    config: {
      type: 'string',
      alias: 'c',
      description:
        'Use this config file (if argument is used but value is unspecified, defaults to `rolldown.config.js`)',
    },
    help: {
      type: 'boolean',
      alias: 'h',
      description: 'Show this help message',
    },
  },
  async run(_ctx) {
    // FIXME: `citty` doesn't support detecting if an argument is unspecified
    const parsedArgs = parseArgs<ParsedArgs>(process.argv.slice(2))
    let argConfig = parsedArgs.c || parsedArgs.config
    if (argConfig) {
      // If config is specified, we will ignore other arguments and bundle with the specified config
      if (argConfig == true) {
        argConfig = DEFAULT_CONFIG_FILENAME
      }
      await bundle(argConfig)
      process.exit(0)
      return
    }

    showUsage(main)
  },
})

runMain(main)
