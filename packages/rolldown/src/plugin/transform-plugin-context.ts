import type {
  BindingPluginContext,
  BindingTransformPluginContext,
} from '../binding';
import {
  type LoggingFunctionWithPosition,
  type LogHandler,
  normalizeLog,
} from '../log/log-handler';
import type { LogLevelOption, RollupError } from '../log/logging';
import { augmentCodeLocation, error, logPluginError } from '../log/logs';
import type { OutputOptions } from '../options/output-options';
import type { Extends, TypeAssert } from '../types/assert';
import type { SourceMap } from '../types/rolldown-output';
import type { Plugin } from './index';
import { type PluginContext, PluginContextImpl } from './plugin-context';
import { PluginContextData } from './plugin-context-data';

export interface TransformPluginContext extends PluginContext {
  debug: LoggingFunctionWithPosition;
  info: LoggingFunctionWithPosition;
  warn: LoggingFunctionWithPosition;
  error(
    e: RollupError | string,
    pos?: number | { column: number; line: number },
  ): never;
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
    super(
      outputOptions,
      context,
      plugin,
      data,
      onLog,
      LogLevelOption,
      watchMode,
      moduleId,
    );
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

  error(
    e: RollupError | string,
    pos?: number | { column: number; line: number },
  ): never {
    if (typeof e === 'string') e = { message: e };
    if (pos) augmentCodeLocation(e, pos, this.moduleSource, this.moduleId);
    e.id = this.moduleId;
    e.hook = 'transform';
    return error(logPluginError(normalizeLog(e), this.pluginName));
  }

  public getCombinedSourcemap(): SourceMap {
    return JSON.parse(this.inner.getCombinedSourcemap());
  }
}

function _assert() {
  // adding implements to class disallows extending PluginContext by declaration merging
  // instead check that TransformPluginContextImpl is assignable to TransformPluginContext here
  type _ = TypeAssert<
    Extends<TransformPluginContextImpl, TransformPluginContext>
  >;
}
