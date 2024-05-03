import { pathToFileURL } from 'node:url'
import nodePath from 'node:path'
import { createConsola } from 'consola'
import { RolldownConfigExport } from '../types/rolldown-config-export.js'

/**
 * Console logger
 */
export const logger = createConsola({
  formatOptions: {
    date: false,
  },
})

export async function ensureConfig(
  configPath: string,
): Promise<RolldownConfigExport> {
  if (!isSupportedFormat(configPath)) {
    throw new Error(
      `Unsupported config format. Expected: \`${SUPPORTED_CONFIG_FORMATS.join(',')}\` but got \`${nodePath.extname(configPath)}\``,
    )
  }

  // Ensure the path is recognized by Node.js in windows
  const fileUrl = pathToFileURL(configPath).toString()

  const configExports = await import(fileUrl)

  // TODO: Could add more validation/diagnostics here to emit a nice error message
  return configExports.default
}

const SUPPORTED_CONFIG_FORMATS = ['.js', '.mjs', '.cjs']

/**
 * Check whether the configuration file is supported
 */
function isSupportedFormat(configPath: string): boolean {
  const ext = nodePath.extname(configPath)
  return SUPPORTED_CONFIG_FORMATS.includes(ext)
}
