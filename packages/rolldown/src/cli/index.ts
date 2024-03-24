import process from 'node:process'
import path from 'node:path'
import consola from 'consola'
import { defineCommand, runMain, showUsage } from 'citty'
import { loadConfig } from './utils.js'
import { bundle } from './commands/bundle'
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
    if (ctx.args.help) {
      await showUsage(ctx.cmd)
      return
    }
    const { configPath } = parseArgs(ctx.args)

    const config = await loadConfig(configPath)

    if (!config) {
      consola.error(`No configuration found at ${configPath}`)
      process.exit(1)
    }

    await bundle(config)
  },
})

function parseArgs(args: Record<string, any>) {
  const { config } = args
  const cwd = process.cwd()
  const configPath = config
    ? path.resolve(cwd, config)
    : path.resolve(cwd, DEFAULT_CONFIG_FILENAME)

  return { configPath }
}

runMain(main)
