import { noop } from '../utils/misc';
import {
  LOG_LEVEL_WARN,
  type LogLevel,
  type LogLevelOption,
  logLevelPriority,
  type RollupLog,
} from './logging';
import { logInvalidLogPosition } from './logs';

export const normalizeLog = (
  log: RollupLog | string | (() => RollupLog | string),
): RollupLog =>
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
  log: RollupLog | string | (() => RollupLog | string),
) => void;

export type LoggingFunctionWithPosition = (
  log: RollupLog | string | (() => RollupLog | string),
  pos?: number | { column: number; line: number },
) => void;

export type LogHandler = (level: LogLevel, log: RollupLog) => void;

export type WarningHandlerWithDefault = (
  warning: RollupLog,
  defaultHandler: LoggingFunction,
) => void;
