import type {
  BindingPluginContext,
  BindingTransformPluginContext,
} from '../binding'
import type { SourceMap } from '../types/rolldown-output'
import type {
  LoggingFunction,
  LoggingFunctionWithPosition,
  RollupError,
} from '../rollup'
import { normalizeLog } from '../log/logHandler'
import type { EmittedAsset, PluginContext } from './plugin-context'
import { augmentCodeLocation } from '../log/logs'

export class TransformPluginContext {
  debug: LoggingFunction
  info: LoggingFunction
  warn: LoggingFunction
  error: (
    error: RollupError | string,
    pos?: number | { column: number; line: number },
  ) => never
  resolve: BindingPluginContext['resolve']
  // getCombinedSourcemap: () => SourceMap
  emitFile: (file: EmittedAsset) => string
  getFileName: (referenceId: string) => string
  parse: (input: string, options?: any) => any

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
    this.parse = context.parse
    // this.getCombinedSourcemap = () => JSON.parse(inner.getCombinedSourcemap())
    this.emitFile = context.emitFile
    this.getFileName = context.getFileName
  }
}
