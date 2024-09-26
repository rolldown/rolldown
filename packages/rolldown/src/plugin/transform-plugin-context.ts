import type {
  BindingPluginContext,
  BindingTransformPluginContext,
} from '../binding'
import type { LoggingFunctionWithPosition, RollupError } from '../rollup'
import { normalizeLog } from '../log/logHandler'
import { PluginContext } from './plugin-context'
import { augmentCodeLocation, error, logPluginError } from '../log/logs'
import { PluginContextData } from './plugin-context-data'
import { NormalizedInputOptions } from '..'
import type { Plugin } from './index'

export class TransformPluginContext extends PluginContext {
  error: (
    error: RollupError | string,
    pos?: number | { column: number; line: number },
  ) => never
  // getCombinedSourcemap: () => SourceMap

  constructor(
    options: NormalizedInputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
    inner: BindingTransformPluginContext,
    moduleId: string,
    moduleSource: string,
  ) {
    super(options, context, plugin, data)
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
    this.error = (
      e: RollupError | string,
      pos?: number | { column: number; line: number },
    ): never => {
      if (typeof e === 'string') e = { message: e }
      if (pos) augmentCodeLocation(e, pos, moduleSource, moduleId)
      e.id = moduleId
      e.hook = 'transform'
      return error(logPluginError(normalizeLog(e), plugin.name || 'unknown'))
    }
    // this.getCombinedSourcemap = () => JSON.parse(inner.getCombinedSourcemap())
  }
}
