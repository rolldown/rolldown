import { defineCommand, runMain, showUsage } from 'citty'
import consola from 'consola'
import process from 'node:process'
import path from 'node:path'
import pkgJson from '../../package.json' assert { type: 'json' }
import { loadConfig } from './utils.js'
import { bundle } from './commands/bundle'

const main = defineCommand({
  meta: {
    name: 'rolldown',
    version: pkgJson.version,
    description: pkgJson.description,
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
    const cwd = process.cwd()
    let configPath
    if (ctx.args.config) {
      configPath = path.resolve(cwd, ctx.args.config)
    } else {
      configPath = path.resolve(cwd, 'rolldown.config.js')
    }

    const config = await loadConfig(configPath)

    if (!config) {
      consola.error(`No configuration found at ${configPath}`)
      process.exit(1)
    }

    await bundle(config)
  },
})

runMain(main)
