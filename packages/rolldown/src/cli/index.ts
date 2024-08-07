import process from 'node:process'
import { defineCommand, runMain } from 'citty'
import { bundle } from './commands/bundle'
import {
  version,
  description,
} from '../../package.json' assert { type: 'json' }

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
      required: true,
      description:
        'Use this config file (if argument is used but value is unspecified, defaults to `rolldown.config.js`)',
    },
    help: {
      type: 'boolean',
      alias: 'h',
      description: 'Show this help message',
    },
  },
  async run(ctx) {
    let config = ctx.args.config
    await bundle(config)
    process.exit(0)
  },
})

runMain(main)
