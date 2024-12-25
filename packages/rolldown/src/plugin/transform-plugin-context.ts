import type {
  BindingPluginContext,
  BindingTransformPluginContext,
} from '../binding'
import type {
  LoggingFunctionWithPosition,
  LogHandler,
  LogLevelOption,
  RollupError,
} from '../types/misc'
import { normalizeLog } from '../log/logHandler'
import { PluginContext } from './plugin-context'
import { augmentCodeLocation, error, logPluginError } from '../log/logs'
import { PluginContextData } from './plugin-context-data'
import type { Plugin } from './index'
import { SourceMap } from '../types/rolldown-output'

export class TransformPluginContext extends PluginContext {
  constructor(
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
    private inner: BindingTransformPluginContext,
    private moduleId: string,
    private moduleSource: string,
    onLog: LogHandler,
    LogLevelOption: LogLevelOption,
  ) {
    super(context, plugin, data, onLog, LogLevelOption, moduleId)
    const getLogHandler =
      (handler: LoggingFunctionWithPosition): LoggingFunctionWithPosition =>
      (log, pos) => {
        log = normalizeLog(log)
        if (pos) augmentCodeLocation(log, pos, moduleSource, moduleId)
        log.id = moduleId
        log.hook = 'transform'
        handler(log)
      }

    this.debug = getLogHandler(this.debug)
    this.warn = getLogHandler(this.warn)
    this.info = getLogHandler(this.info)
  }

  error(
    e: RollupError | string,
    pos?: number | { column: number; line: number },
  ): never {
    if (typeof e === 'string') e = { message: e }
    if (pos) augmentCodeLocation(e, pos, this.moduleSource, this.moduleId)
    e.id = this.moduleId
    e.hook = 'transform'
    return error(logPluginError(normalizeLog(e), this.pluginName))
  }

  public getCombinedSourcemap(): SourceMap {
    return JSON.parse(this.inner.getCombinedSourcemap())
  }
}
