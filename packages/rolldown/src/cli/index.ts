import { defineCommand, runMain, showUsage } from 'citty'
import { bundle } from './commands/bundle.js'
import {
  version,
  description,
} from '../../package.json' assert { type: 'json' }
import { DEFAULT_CONFIG_FILENAME } from './constants.js'

const main = defineCommand({
  meta: {
    name: 'rolldown',
    version,
    description,
  },
  args: {
    config: {
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
    let argConfig = _ctx.args.config
    if (typeof argConfig === 'string' || argConfig === true) {
      // If config is specified, we will ignore other arguments and bundle with the specified config
      if (argConfig === true) {
        argConfig = DEFAULT_CONFIG_FILENAME
      }
      await bundle(argConfig)
      return
    }

    showUsage(main)
  },
})

runMain(main)
