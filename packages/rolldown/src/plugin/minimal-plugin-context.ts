import type { LoggingFunction, LogHandler, RollupError } from '../rollup'
import type { Plugin } from '../plugin'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { error, logPluginError } from '../log/logs'
import { getLogHandler, normalizeLog } from '../log/logHandler'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import { VERSION } from '..'

export interface PluginContextMeta {
  rollupVersion: string
  rolldownVersion: string
  watchMode: boolean
}

export class MinimalPluginContext {
  info: LoggingFunction
  warn: LoggingFunction
  debug: LoggingFunction
  meta: PluginContextMeta
  readonly error: (error: RollupError | string) => never

  constructor(options: NormalizedInputOptions, plugin: Plugin) {
    const onLog = options.onLog as LogHandler
    const pluginName = plugin.name || 'unknown'
    const logLevel = options.logLevel || LOG_LEVEL_INFO
    this.debug = getLogHandler(
      LOG_LEVEL_DEBUG,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    )
    this.info = getLogHandler(
      LOG_LEVEL_INFO,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    )
    this.warn = getLogHandler(
      LOG_LEVEL_WARN,
      'PLUGIN_WARNING',
      onLog,
      pluginName,
      logLevel,
    )
    this.error = (e): never => {
      return error(logPluginError(normalizeLog(e), pluginName))
    }
    this.meta = {
      rollupVersion: '4.23.0',
      rolldownVersion: VERSION,
      watchMode: false,
    }
  }
}
