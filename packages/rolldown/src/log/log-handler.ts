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

export type LoggingFunction = (log: RolldownLog | string | (() => RolldownLog | string)) => void;

export type LoggingFunctionWithPosition = (
  log: RolldownLog | string | (() => RolldownLog | string),
  pos?: number | { column: number; line: number },
) => void;

export type LogHandler = (level: LogLevel, log: RolldownLog) => void;

export type WarningHandlerWithDefault = (
  warning: RolldownLog,
  defaultHandler: LoggingFunction,
) => void;
