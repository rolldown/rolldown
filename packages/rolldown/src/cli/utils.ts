import { pathToFileURL } from 'node:url'
import nodePath from 'node:path'
import { createConsola } from 'consola'
import { ERR_UNSUPPORTED_CONFIG_FORMAT } from './errors.js'
import { RolldownConfigExport } from '../types/rolldown-config-export.js'

/**
 * Console logger
 */
export const logger = createConsola({
  formatOptions: {
    date: false,
  },
})

/**
 * Load a rolldown configuration file
 */
export async function loadConfig(
  configPath: string,
): Promise<RolldownConfigExport | undefined> {
  if (!isSupportedFormat(configPath)) {
    throw new Error(ERR_UNSUPPORTED_CONFIG_FORMAT)
  }
  return import(pathToFileURL(configPath).toString()).then(
    (config) => config.default,
  )
}

/**
 * Check whether the configuration file is supported
 */
function isSupportedFormat(configPath: string): boolean {
  const ext = nodePath.extname(configPath)
  return /\.(js|mjs)$/.test(ext)
}
