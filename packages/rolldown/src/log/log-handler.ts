import { noop } from '../utils/misc';
import {
  LOG_LEVEL_WARN,
  type LogLevel,
  type LogLevelOption,
  logLevelPriority,
  type RolldownLog,
} from './logging';
import { logInvalidLogPosition } from './logs';

export const normalizeLog = (
  log: RolldownLog | string | (() => RolldownLog | string),
): RolldownLog =>
  typeof log === 'string'
    ? { message: log }
    : typeof log === 'function'
      ? normalizeLog(log())
      : log;

export function getLogHandler(
  level: LogLevel,
  code: string,
  logger: LogHandler,
  pluginName: string,
  logLevel: LogLevelOption,
): LoggingFunctionWithPosition {
  if (logLevelPriority[level] < logLevelPriority[logLevel]) {
    return noop;
  }
  return (log, pos) => {
    if (pos != null) {
      logger(LOG_LEVEL_WARN, logInvalidLogPosition(pluginName));
    }
    log = normalizeLog(log);
    if (log.code && !log.pluginCode) {
      log.pluginCode = log.code;
    }
    log.code = code;
    log.plugin = pluginName;
    logger(level, log);
  };
}

export type LoggingFunction = (
  /**
   * The log object or message.
   *
   * The string argument is equivalent to passing an object with only the
   * {@linkcode RolldownLog.message | message} property.
   */
  log: RolldownLog | string | (() => RolldownLog | string),
) => void;

export type LoggingFunctionWithPosition = (
  log: RolldownLog | string | (() => RolldownLog | string),
  /**
   * A character index or file location which will be used to augment the log with
   * {@linkcode RolldownLog.pos | pos}, {@linkcode RolldownLog.loc | loc} and
   * {@linkcode RolldownLog.frame | frame}.
   */
  pos?: number | { column: number; line: number },
) => void;

export type LogHandler = (level: LogLevel, log: RolldownLog) => void;

export type WarningHandlerWithDefault = (
  warning: RolldownLog,
  defaultHandler: LoggingFunction,
) => void;
