import { RollupError, LoggingFunction } from '../rollup'
import { BindingPluginContext } from '../binding'
import { getLogHandler, normalizeLog } from '../log/logHandler'
import { NormalizedInputOptions } from '../options/normalized-input-options'
import type { Plugin } from './index'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { error, logPluginError } from '../log/logs'

export class PluginContext {
  debug: LoggingFunction
  info: LoggingFunction
  warn: LoggingFunction
  error: (error: RollupError | string) => never
  resolve: BindingPluginContext['resolve']

  constructor(
    options: NormalizedInputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
  ) {
    // TODO add `onLog` option
    const onLog = () => {}
    const pluginName = plugin.name || 'unknown'
    const logLevel = options.logLevel || LOG_LEVEL_INFO
    this.debug = getLogHandler(
      LOG_LEVEL_DEBUG,
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
    this.info = getLogHandler(
      LOG_LEVEL_INFO,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    )
    this.error = (e): never => {
      return error(logPluginError(normalizeLog(e), pluginName))
    }
    this.resolve = context.resolve.bind(context)
  }
}
