import type {
  BindingPluginContext,
  BindingTransformPluginContext,
} from '@src/binding'
import type { SourceMap } from '@src/types/rolldown-output'
import type {
  LoggingFunction,
  LoggingFunctionWithPosition,
  RollupError,
} from '../rollup'
import { normalizeLog } from '@src/log/logHandler'
import type { PluginContext } from './plugin-context'
import { augmentCodeLocation } from '@src/log/logs'

export class TransformPluginContext {
  debug: LoggingFunction
  info: LoggingFunction
  warn: LoggingFunction
  error: (error: RollupError | string) => never
  resolve: BindingPluginContext['resolve']
  getCombinedSourcemap: () => SourceMap

  constructor(
    inner: BindingTransformPluginContext,
    context: PluginContext,
    moduleId: string,
    moduleSource: string,
  ) {
    const getLogHandler =
      (handler: LoggingFunctionWithPosition): LoggingFunctionWithPosition =>
      (log, pos) => {
        log = normalizeLog(log)
        if (pos) augmentCodeLocation(log, pos, moduleSource, moduleId)
        log.id = moduleId
        log.hook = 'transform'
        handler(log)
      }

    this.debug = getLogHandler(context.debug)
    this.warn = getLogHandler(context.warn)
    this.info = getLogHandler(context.info)
    this.error = (
      error: RollupError | string,
      pos?: number | { column: number; line: number },
    ): never => {
      if (typeof error === 'string') error = { message: error }
      if (pos) augmentCodeLocation(error, pos, moduleSource, moduleId)
      error.id = moduleId
      error.hook = 'transform'
      return context.error(error)
    }
    this.resolve = context.resolve
    this.getCombinedSourcemap = () => JSON.parse(inner.getCombinedSourcemap())
  }
}
