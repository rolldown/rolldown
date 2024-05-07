import { RollupError, LoggingFunction } from '../rollup'
import { BindingPluginContext } from '../binding'
import { getLogHandler, normalizeLog } from '../log/logHandler'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import type { Plugin } from './index'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { error, logPluginError } from '../log/logs'

export interface PluginContext {
  debug: LoggingFunction
  info: LoggingFunction
  warn: LoggingFunction
  error: (error: RollupError | string) => never
}

export function transformPluginContext(
  options: NormalizedInputOptions,
  context: BindingPluginContext,
  plugin: Plugin,
): PluginContext {
  const { onLog } = options
  const pluginName = plugin.name || 'unknown'
  const logLevel = options.logLevel || LOG_LEVEL_INFO
  return {
    ...context,
    debug: getLogHandler(
      LOG_LEVEL_DEBUG,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    ),
    warn: getLogHandler(
      LOG_LEVEL_WARN,
      'PLUGIN_WARNING',
      onLog,
      pluginName,
      logLevel,
    ),
    info: getLogHandler(
      LOG_LEVEL_INFO,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    ),
    error(e): never {
      return error(logPluginError(normalizeLog(e), pluginName))
    },
  }
}
