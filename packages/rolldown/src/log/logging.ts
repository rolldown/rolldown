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
   *
   * For errors / warnings / logs from plugins, it is always `PLUGIN_ERROR` / `PLUGIN_WARNING` / `PLUGIN_LOG`. If a plugin sets a `code` to identify a specific warning, Rolldown copies that code to `pluginCode`.
   *
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
  /**
   * The name of the plugin that caused this log.
   *
   * This is set by Rolldown automatically when a plugin causes a log.
   */
  plugin?: string;
  /**
   * The code set by the plugin that caused this log.
   */
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
