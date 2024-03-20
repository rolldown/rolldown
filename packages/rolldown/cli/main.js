import { defineCommand, runMain, showUsage } from 'citty'
import consola from 'consola'
import process from 'node:process'
import path from 'node:path'
import pkgJson from '../package.json' assert { type: 'json' }
import { normalizeConfigPath, loadConfig } from './config.js'
import build from './build.js'

/**
 * NOTE:
 *  currently, It's hard to customize usage with citty `renderUsage`.
 *  It may be better to use another CLI library or construct our own.
 */

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
  },
  async run(ctx) {
    if (ctx.rawArgs.length === 0) {
      await showUsage(ctx.cmd)
      return
    }

    const currentDir = path.resolve(process.cwd(), '.')
    const configPath = normalizeConfigPath(
      path.resolve(currentDir, ctx.args.config),
    )
    const { default: config } = loadConfig(configPath)
    consola.debug('loaded config', config)

    await build(config)
  },
})

runMain(main)
