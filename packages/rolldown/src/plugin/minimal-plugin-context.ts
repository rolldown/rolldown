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
  type RolldownError,
} from '../log/logging';
import { error, logPluginError } from '../log/logs';
import type { Extends, TypeAssert } from '../types/assert';
import { VERSION } from '../constants';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { RolldownLog } from '../log/logging';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { Plugin } from '../plugin/index';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { InputOptions } from '../options/input-options';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { watch } from '../api/watch/index';

/** @category Plugin APIs */
export interface PluginContextMeta {
  /**
   * A property for Rollup compatibility. A dummy value is set by Rolldown.
   * @example `'4.23.0'`
   */
  rollupVersion: string;
  /**
   * The currently running version of Rolldown.
   * @example `'1.0.0'`
   */
  rolldownVersion: string;
  /**
   * Whether Rolldown was started via {@linkcode watch | rolldown.watch()} or
   * from the command line with `--watch`.
   */
  watchMode: boolean;
}

/** @category Plugin APIs */
export interface MinimalPluginContext {
  /** @hidden */
  readonly pluginName: string;
  /**
   * Similar to {@linkcode warn | this.warn}, except that it will also abort
   * the bundling process with an error.
   *
   * If an Error instance is passed, it will be used as-is, otherwise a new Error
   * instance will be created with the given error message and all additional
   * provided properties.
   *
   * In all hooks except the {@linkcode Plugin.onLog | onLog} hook, the error will
   * be augmented with {@linkcode RolldownLog.code | code: "PLUGIN_ERROR"} and
   * {@linkcode RolldownLog.plugin | plugin: plugin.name} properties.
   * If a `code` property already exists and the code does not start with `PLUGIN_`,
   * it will be renamed to {@linkcode RolldownLog.pluginCode | pluginCode}.
   *
   * @group Logging Methods
   */
  error: (e: RolldownError | string) => never;
  /**
   * Generate a `"info"` level log.
   *
   * {@linkcode RolldownLog.code | code} will be set to `"PLUGIN_LOG"` by Rolldown.
   * As these logs are displayed by default, use them for information that is not a warning
   * but makes sense to display to all users on every build.
   *
   * {@include ./docs/plugin-context-info.md}
   *
   * @inlineType LoggingFunction
   * @group Logging Methods
   */
  info: LoggingFunction;
  /**
   * Generate a `"warn"` level log.
   *
   * Just like internally generated warnings, these logs will be first passed to and
   * filtered by plugin {@linkcode Plugin.onLog | onLog} hooks before they are forwarded
   * to custom {@linkcode InputOptions.onLog | onLog} or
   * {@linkcode InputOptions.onwarn | onwarn} handlers or printed to the console.
   *
   * We encourage you to use objects with a {@linkcode RolldownLog.pluginCode | pluginCode}
   * property as that will allow users to easily filter for those logs in an `onLog` handler.
   *
   * {@include ./docs/plugin-context-warn.md}
   *
   * @inlineType LoggingFunction
   * @group Logging Methods
   */
  warn: LoggingFunction;
  /**
   * Generate a `"debug"` level log.
   *
   * {@linkcode RolldownLog.code | code} will be set to `"PLUGIN_LOG"` by Rolldown.
   * Make sure to add a distinctive {@linkcode RolldownLog.pluginCode | pluginCode} to
   * those logs for easy filtering.
   *
   * {@include ./docs/plugin-context-debug.md}
   *
   * @inlineType LoggingFunction
   * @group Logging Methods
   */
  debug: LoggingFunction;
  /** An object containing potentially useful metadata. */
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
    this.debug = getLogHandler(LOG_LEVEL_DEBUG, 'PLUGIN_LOG', onLog, pluginName, logLevel);
    this.info = getLogHandler(LOG_LEVEL_INFO, 'PLUGIN_LOG', onLog, pluginName, logLevel);
    this.warn = getLogHandler(LOG_LEVEL_WARN, 'PLUGIN_WARNING', onLog, pluginName, logLevel);

    this.meta = {
      rollupVersion: '4.23.0',
      rolldownVersion: VERSION,
      watchMode,
    };
  }

  public error(e: RolldownError | string): never {
    return error(logPluginError(normalizeLog(e), this.pluginName, { hook: this.hookName }));
  }
}

function _assert() {
  // adding implements to class disallows extending PluginContext by declaration merging
  // instead check that MinimalPluginContextImpl is assignable to MinimalPluginContext here
  type _ = TypeAssert<Extends<MinimalPluginContextImpl, MinimalPluginContext>>;
}
