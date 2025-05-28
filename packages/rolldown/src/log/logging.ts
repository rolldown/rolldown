export type LogLevel = 'info' | 'debug' | 'warn';
export type LogLevelOption = LogLevel | 'silent';
export type LogLevelWithError = LogLevel | 'error';

export interface RollupLog {
  binding?: string;
  cause?: unknown;
  code?: string;
  exporter?: string;
  frame?: string;
  hook?: string;
  id?: string;
  ids?: string[];
  loc?: {
    column: number;
    file?: string;
    line: number;
  };
  message: string;
  meta?: any;
  names?: string[];
  plugin?: string;
  pluginCode?: unknown;
  pos?: number;
  reexporter?: string;
  stack?: string;
  url?: string;
}

export type RollupLogWithString = RollupLog | string;

export interface RollupError extends RollupLog {
  name?: string;
  stack?: string;
  watchFiles?: string[];
}

export type LogOrStringHandler = (
  level: LogLevelWithError,
  log: RollupLogWithString,
) => void;

const LOG_LEVEL_SILENT: LogLevelOption = 'silent';
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
