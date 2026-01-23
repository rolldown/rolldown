/** @inline */
export type LogLevel = 'info' | 'debug' | 'warn';
/** @inline */
export type LogLevelOption = LogLevel | 'silent';
/** @inline */
export type LogLevelWithError = LogLevel | 'error';

export interface RolldownLog {
  binding?: string;
  cause?: unknown;
  /**
   * The log code for this log object.
   * @example 'PLUGIN_ERROR'
   */
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
  /**
   * The message for this log object.
   * @example 'The "transform" hook used by the output plugin "rolldown-plugin-foo" is a build time hook and will not be run for that plugin. Either this plugin cannot be used as an output plugin, or it should have an option to configure it as an output plugin.'
   */
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

/** @inline */
export type RolldownLogWithString = RolldownLog | string;

/** @category Plugin APIs */
export interface RolldownError extends RolldownLog {
  name?: string;
  stack?: string;
  watchFiles?: string[];
}

export type LogOrStringHandler = (level: LogLevelWithError, log: RolldownLogWithString) => void;

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
