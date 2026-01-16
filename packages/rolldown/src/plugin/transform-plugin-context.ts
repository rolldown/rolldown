import type {
  BindingMagicString,
  BindingPluginContext,
  BindingTransformPluginContext,
} from '../binding.cjs';
import {
  type LoggingFunctionWithPosition,
  type LogHandler,
  normalizeLog,
} from '../log/log-handler';
import type { LogLevelOption, RolldownError } from '../log/logging';
import { augmentCodeLocation, error, logPluginError } from '../log/logs';
import type { OutputOptions } from '../options/output-options';
import type { Extends, TypeAssert } from '../types/assert';
import type { SourceMap } from '../types/rolldown-output';
import type { Plugin } from './index';
import { type PluginContext, PluginContextImpl } from './plugin-context';
import type { PluginContextData } from './plugin-context-data';
// oxlint-disable-next-line no-unused-vars -- this is used in JSDoc links
import type { RolldownLog } from '../log/logging';

/** @category Plugin APIs */
export interface TransformPluginContext extends PluginContext {
  /**
   * Same as {@linkcode PluginContext.debug}, but a `position` param can be supplied.
   *
   * @inlineType LoggingFunctionWithPosition
   * @group Logging Methods
   */
  debug: LoggingFunctionWithPosition;
  /**
   * Same as {@linkcode PluginContext.info}, but a `position` param can be supplied.
   *
   * @inlineType LoggingFunctionWithPosition
   * @group Logging Methods
   */
  info: LoggingFunctionWithPosition;
  /**
   * Same as {@linkcode PluginContext.warn}, but a `position` param can be supplied.
   *
   * @inlineType LoggingFunctionWithPosition
   * @group Logging Methods
   */
  warn: LoggingFunctionWithPosition;
  /**
   * Same as {@linkcode PluginContext.error}, but the `id` of the current module will
   * also be added and a `position` param can be supplied.
   */
  error(
    e: RolldownError | string,
    /**
     * A character index or file location which will be used to augment the log with
     * {@linkcode RolldownLog.pos | pos}, {@linkcode RolldownLog.loc | loc} and
     * {@linkcode RolldownLog.frame | frame}.
     */
    pos?: number | { column: number; line: number },
  ): never;
  /**
   * Get the combined source maps of all previous plugins.
   */
  getCombinedSourcemap(): SourceMap;
}

export class TransformPluginContextImpl extends PluginContextImpl {
  constructor(
    outputOptions: OutputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
    private inner: BindingTransformPluginContext,
    private moduleId: string,
    private moduleSource: string,
    onLog: LogHandler,
    LogLevelOption: LogLevelOption,
    watchMode: boolean,
  ) {
    super(outputOptions, context, plugin, data, onLog, LogLevelOption, watchMode, moduleId);
    const getLogHandler =
      (handler: LoggingFunctionWithPosition): LoggingFunctionWithPosition =>
      (log, pos) => {
        log = normalizeLog(log);
        if (pos) augmentCodeLocation(log, pos, moduleSource, moduleId);
        log.id = moduleId;
        log.hook = 'transform';
        handler(log);
      };

    this.debug = getLogHandler(this.debug);
    this.warn = getLogHandler(this.warn);
    this.info = getLogHandler(this.info);
  }

  error(e: RolldownError | string, pos?: number | { column: number; line: number }): never {
    if (typeof e === 'string') e = { message: e };
    if (pos) augmentCodeLocation(e, pos, this.moduleSource, this.moduleId);
    e.id = this.moduleId;
    e.hook = 'transform';
    return error(logPluginError(normalizeLog(e), this.pluginName));
  }

  public getCombinedSourcemap(): SourceMap {
    return JSON.parse(this.inner.getCombinedSourcemap());
  }

  public addWatchFile(id: string): void {
    this.inner.addWatchFile(id);
  }

  public sendMagicString(s: BindingMagicString): void {
    this.inner.sendMagicString(s);
  }
}

function _assert() {
  // adding implements to class disallows extending PluginContext by declaration merging
  // instead check that TransformPluginContextImpl is assignable to TransformPluginContext here
  type _ = TypeAssert<Extends<TransformPluginContextImpl, TransformPluginContext>>;
}
