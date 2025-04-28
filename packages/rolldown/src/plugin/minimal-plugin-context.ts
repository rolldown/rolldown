import { VERSION } from '..';
import {
  getLogHandler,
  type LoggingFunction,
  type LogHandler,
  normalizeLog,
} from '../log/log-handler';
import {
  LOG_LEVEL_DEBUG,
  LOG_LEVEL_INFO,
  LOG_LEVEL_WARN,
  type LogLevelOption,
  type RollupError,
} from '../log/logging';
import { error, logPluginError } from '../log/logs';
import type { Extends, TypeAssert } from '../types/assert';

export interface PluginContextMeta {
  rollupVersion: string;
  rolldownVersion: string;
  watchMode: boolean;
}

export interface MinimalPluginContext {
  readonly pluginName: string;
  error: (e: RollupError | string) => never;
  info: LoggingFunction;
  warn: LoggingFunction;
  debug: LoggingFunction;
  meta: PluginContextMeta;
}

export class MinimalPluginContextImpl {
  info: LoggingFunction;
  warn: LoggingFunction;
  debug: LoggingFunction;
  meta: PluginContextMeta;

  constructor(
    onLog: LogHandler,
    logLevel: LogLevelOption,
    readonly pluginName: string,
    watchMode: boolean,
    private readonly hookName?: string,
  ) {
    this.debug = getLogHandler(
      LOG_LEVEL_DEBUG,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    );
    this.info = getLogHandler(
      LOG_LEVEL_INFO,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    );
    this.warn = getLogHandler(
      LOG_LEVEL_WARN,
      'PLUGIN_WARNING',
      onLog,
      pluginName,
      logLevel,
    );

    this.meta = {
      rollupVersion: '4.23.0',
      rolldownVersion: VERSION,
      watchMode,
    };
  }

  public error(e: RollupError | string): never {
    return error(
      logPluginError(normalizeLog(e), this.pluginName, { hook: this.hookName }),
    );
  }
}

function _assert() {
  // adding implements to class disallows extending PluginContext by declaration merging
  // instead check that MinimalPluginContextImpl is assignable to MinimalPluginContext here
  type _ = TypeAssert<Extends<MinimalPluginContextImpl, MinimalPluginContext>>;
}
