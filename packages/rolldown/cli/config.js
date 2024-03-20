import jitiFactory from 'jiti'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { ERR_UNSUPPORTED_CONFIG_FORMAT } from './errors.js'

const __filename = fileURLToPath(import.meta.url)

/**
 * @typedef {import('../src/rollup.d.ts').RollupOptions} RollupOptions
 */

/**
 * Load a rolldown configuration file
 *
 * @param {string} configPath - A path of rolldown configuration file
 * @returns {RollupOptions | RollupOptions[]} A rollup options via rollup configuration file
 */
export function loadConfig(configPath) {
  if (!isSupportedFormat(configPath)) {
    throw new Error(ERR_UNSUPPORTED_CONFIG_FORMAT)
  }
  return lazyJiti()(configPath)
}

/**
 * Check whether the configuration file is supported
 *
 * @param {string} configPath - The path of the rolldown configuration file
 * @returns {boolean} whether the configuration file is supported
 */
function isSupportedFormat(configPath) {
  const ext = path.extname(configPath)
  return /\.(js|mjs|ts)$/.test(ext)
}

/**
 * @type {import('jiti').JITI | null}
 */
let jiti = null

/**
 * Get a jiti instance lazily
 * @returns {import('jiti').JITI}
 */
function lazyJiti() {
  return jiti ?? (jiti = jitiFactory(__filename))
}
