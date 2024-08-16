import { pathToFileURL } from 'node:url'
import nodePath from 'node:path'
import { createConsola } from 'consola'
import type { ConfigExport } from '../types/config-export'

/**
 * Console logger
 */
export const logger = process.env.ROLLDOWN_TEST
  ? createTestingLogger()
  : createConsola({
      formatOptions: {
        date: false,
      },
    })

function createTestingLogger() {
  const types = [
    'silent',
    'fatal',
    'error',
    'warn',
    'log',
    'info',
    'success',
    'fail',
    'ready',
    'start',
    'box',
    'debug',
    'trace',
    'verbose',
  ]
  const ret: Record<string, any> = Object.create(null)
  for (const type of types) {
    ret[type] = console.log
  }
  return ret
}

export async function ensureConfig(configPath: string): Promise<ConfigExport> {
  if (!isSupportedFormat(configPath)) {
    throw new Error(
      `Unsupported config format. Expected: \`${SUPPORTED_CONFIG_FORMATS.join(
        ',',
      )}\` but got \`${nodePath.extname(configPath)}\``,
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
