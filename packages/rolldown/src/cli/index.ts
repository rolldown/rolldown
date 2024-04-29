import process from 'node:process'
import nodePath from 'node:path'
import { defineCommand, runMain, showUsage as _showUsage } from 'citty'
import { logger, loadConfig } from './utils.js'
import { bundle } from './commands/bundle.js'
import { showUsage } from './usage.js'
import {
  version,
  description,
} from '../../package.json' assert { type: 'json' }

const DEFAULT_CONFIG_FILENAME = 'rolldown.config.js'

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
        'Use this config file (if argument is used but value is unspecified, defaults to rolldown.config.js)',
    },
    help: {
      type: 'boolean',
      alias: 'h',
      description: 'Show this help message',
    },
  },
  async run(ctx) {
    const { configPath } = parseArgs(ctx.args)

    const config = await loadConfig(configPath)

    if (!config) {
      logger.error(`No configuration found at ${configPath}`)
      process.exit(1)
    }

    await bundle(config)
  },
})

function parseArgs(args: Record<string, any>) {
  const { config } = args
  const cwd = process.cwd()
  const configPath = config
    ? nodePath.resolve(cwd, config)
    : nodePath.resolve(cwd, DEFAULT_CONFIG_FILENAME)

  return { configPath }
}

runMain(main, { showUsage })
