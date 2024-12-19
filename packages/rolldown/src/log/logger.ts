import type {
  LoggingFunction,
  LogHandler,
  RollupError,
  RollupLog,
  WarningHandlerWithDefault,
} from '../types/misc'
import type { Plugin } from '../plugin'
import {
  LOG_LEVEL_DEBUG,
  LOG_LEVEL_INFO,
  LOG_LEVEL_WARN,
  LOG_LEVEL_ERROR,
  logLevelPriority,
  type LogLevelOption,
  type LogLevel,
} from './logging'
import { error } from './logs'
import { normalizeLog } from './logHandler'
import type { InputOptions } from '../options/input-options'
import path from 'node:path'
import { VERSION } from '..'
import { getSortedPlugins } from '../plugin/plugin-driver'

export function getLogger(
  plugins: Plugin[],
  onLog: LogHandler,
  logLevel: LogLevelOption,
): LogHandler {
  const minimalPriority = logLevelPriority[logLevel]
  const logger = (
    level: LogLevel,
    log: RollupLog,
    skipped: ReadonlySet<Plugin> = new Set(),
  ) => {
    const logPriority = logLevelPriority[level]
    if (logPriority < minimalPriority) {
      return
    }
    for (const plugin of getSortedPlugins('onLog', plugins)) {
      if (skipped.has(plugin)) continue

      const { onLog: pluginOnLog } = plugin

      if (pluginOnLog) {
        const getLogHandler = (level: LogLevel): LoggingFunction => {
          if (logLevelPriority[level] < minimalPriority) {
            return () => {}
          }
          return (log) =>
            logger(level, normalizeLog(log), new Set(skipped).add(plugin))
        }

        const handler =
          'handler' in pluginOnLog! ? pluginOnLog.handler : pluginOnLog!
        if (
          handler.call(
            {
              debug: getLogHandler(LOG_LEVEL_DEBUG),
              error: (log: RollupError | string): never =>
                error(normalizeLog(log)),
              info: getLogHandler(LOG_LEVEL_INFO),
              meta: {
                rollupVersion: '4.23.0',
                rolldownVersion: VERSION,
                watchMode: false,
              },
              warn: getLogHandler(LOG_LEVEL_WARN),
              pluginName: plugin.name || 'unknown',
            },
            level,
            log,
          ) === false
        ) {
          return
        }
      }
    }
    onLog(level, log)
  }

  return logger
}

export const getOnLog = (
  config: InputOptions,
  logLevel: LogLevelOption,
  printLog: LogHandler = defaultPrintLog,
): LogHandler => {
  const { onwarn, onLog } = config
  const defaultOnLog = getDefaultOnLog(printLog, onwarn)
  if (onLog) {
    const minimalPriority = logLevelPriority[logLevel]
    return (level, log) =>
      onLog(level, addLogToString(log), (level, handledLog) => {
        if (level === LOG_LEVEL_ERROR) {
          return error(normalizeLog(handledLog))
        }
        if (logLevelPriority[level] >= minimalPriority) {
          defaultOnLog(level, normalizeLog(handledLog))
        }
      })
  }
  return defaultOnLog
}

const getDefaultOnLog = (
  printLog: LogHandler,
  onwarn?: WarningHandlerWithDefault,
): LogHandler =>
  onwarn
    ? (level, log) => {
        if (level === LOG_LEVEL_WARN) {
          onwarn(addLogToString(log), (warning) =>
            printLog(LOG_LEVEL_WARN, normalizeLog(warning)),
          )
        } else {
          printLog(level, log)
        }
      }
    : printLog

const addLogToString = (log: RollupLog): RollupLog => {
  Object.defineProperty(log, 'toString', {
    value: () => getExtendedLogMessage(log),
    writable: true,
  })
  return log
}

const defaultPrintLog: LogHandler = (level, log) => {
  const message = getExtendedLogMessage(log)
  switch (level) {
    case LOG_LEVEL_WARN: {
      return console.warn(message)
    }
    case LOG_LEVEL_DEBUG: {
      return console.debug(message)
    }
    default: {
      return console.info(message)
    }
  }
}

const getExtendedLogMessage = (log: RollupLog): string => {
  let prefix = ''

  if (log.plugin) {
    prefix += `(${log.plugin} plugin) `
  }
  if (log.loc) {
    prefix += `${relativeId(log.loc.file!)} (${log.loc.line}:${log.loc.column}) `
  }

  return prefix + log.message
}

function relativeId(id: string): string {
  if (!path.isAbsolute(id)) return id
  return path.relative(path.resolve(), id)
}
