import type {
  LoggingFunction,
  LogHandler,
  LogLevelOption,
  RollupError,
} from '../rollup'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { error, logPluginError } from '../log/logs'
import { getLogHandler, normalizeLog } from '../log/logHandler'
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

  constructor(
    onLog: LogHandler,
    logLevel: LogLevelOption,
    readonly pluginName: string,
  ) {
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

    this.meta = {
      rollupVersion: '4.23.0',
      rolldownVersion: VERSION,
      watchMode: false,
    }
  }

  public error(e: RollupError | string): never {
    return error(logPluginError(normalizeLog(e), this.pluginName))
  }
}
