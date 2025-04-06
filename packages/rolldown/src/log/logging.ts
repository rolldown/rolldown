export type LogLevel = 'info' | 'debug' | 'warn';
export type LogLevelOption = LogLevel | 'silent';
export type LogLevelWithError = LogLevel | 'error';

// TODO RollupLog Fields
export type RollupLog = any;
export type RollupLogWithString = RollupLog | string;

export type LogOrStringHandler = (
  level: LogLevelWithError,
  log: RollupLogWithString,
) => void;

export const LOG_LEVEL_SILENT: LogLevelOption = 'silent';
export const LOG_LEVEL_ERROR = 'error';
export const LOG_LEVEL_WARN: LogLevel = 'warn';
export const LOG_LEVEL_INFO: LogLevel = 'info';
export const LOG_LEVEL_DEBUG: LogLevel = 'debug';

export const logLevelPriority: Record<LogLevelOption, number> = {
  [LOG_LEVEL_DEBUG]: 0,
  [LOG_LEVEL_INFO]: 1,
  [LOG_LEVEL_WARN]: 2,
  [LOG_LEVEL_SILENT]: 3,
};
