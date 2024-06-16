import { getCodeFrame } from '../utils/code-frame'
import { locate } from 'locate-character'
import type { RollupLog } from '../rollup'

const INVALID_LOG_POSITION = 'INVALID_LOG_POSITION',
  PLUGIN_ERROR = 'PLUGIN_ERROR'

export function logInvalidLogPosition(pluginName: string): RollupLog {
  return {
    code: INVALID_LOG_POSITION,
    message: `Plugin "${pluginName}" tried to add a file position to a log or warning. This is only supported in the "transform" hook at the moment and will be ignored.`,
  }
}

export function logPluginError(
  error: Omit<RollupLog, 'code'> & { code?: unknown },
  plugin: string,
  { hook, id }: { hook?: string; id?: string } = {},
) {
  const code = error.code
  if (
    !error.pluginCode &&
    code != null &&
    (typeof code !== 'string' || !code.startsWith('PLUGIN_'))
  ) {
    error.pluginCode = code
  }
  error.code = PLUGIN_ERROR
  error.plugin = plugin
  if (hook) {
    error.hook = hook
  }
  if (id) {
    error.id = id
  }
  return error as RollupLog
}

export function error(base: Error | RollupLog): never {
  if (!(base instanceof Error)) {
    base = Object.assign(new Error(base.message), base)
    Object.defineProperty(base, 'name', {
      value: 'RollupError',
      writable: true,
    })
  }
  throw base
}

export function augmentCodeLocation(
  properties: RollupLog,
  pos: number | { column: number; line: number },
  source: string,
  id: string,
): void {
  if (typeof pos === 'object') {
    const { line, column } = pos
    properties.loc = { column, file: id, line }
  } else {
    properties.pos = pos
    const location = locate(source, pos, { offsetLine: 1 })
    if (!location) {
      return
    }
    const { line, column } = location
    properties.loc = { column, file: id, line }
  }

  if (properties.frame === undefined) {
    const { line, column } = properties.loc
    properties.frame = getCodeFrame(source, line, column)
  }
}
