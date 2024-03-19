import { defineCommand, runMain, showUsage } from 'citty'
import { colors } from 'consola/utils'
import path from 'node:path'
import { URL } from 'node:url'
import { getPackageJSON } from './utils.js'

const __dirname = path.dirname(new URL(import.meta.url).pathname)
const { version, description } = getPackageJSON(path.resolve(__dirname, '..'))

/**
 * NOTE:
 *  currenctly, It's hard to customize usage with citty `renderUsage`.
 *  It may be better to use another CLI library or construct our own.
 */

const main = defineCommand({
  meta: {
    name: 'rolldown',
    version,
    description,
  },
  args: {
    config: {
      type: 'string',
      description:
        'Use this config file (if argument is used but value is unspecified, defaults to rolldown.config.js)',
    },
  },
  async run(ctx) {
    if (ctx.rawArgs.length === 0) {
      await showUsage(ctx.cmd)
      return
    }
  },
})

runMain(main)
