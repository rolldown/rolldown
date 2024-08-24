import type { RollupError, LoggingFunction } from '../rollup'
import type { BindingPluginContext } from '../binding'
import { getLogHandler, normalizeLog } from '../log/logHandler'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type {
  CustomPluginOptions,
  ModuleOptions,
  Plugin,
  ResolvedId,
} from './index'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { error, logPluginError } from '../log/logs'
import { AssetSource, bindingAssetSource } from '../utils/asset-source'
import { unimplemented, unsupported } from '../utils/misc'
import { ModuleInfo } from '../types/module-info'
import { PluginContextData } from './plugin-context-data'
import { SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF } from '../constants/plugin-context'

export interface EmittedAsset {
  type: 'asset'
  name?: string
  fileName?: string
  originalFileName?: string | null
  source: AssetSource
}

export type EmittedFile = EmittedAsset

export interface PluginContextResolveOptions {
  skipSelf?: boolean
  custom?: CustomPluginOptions
}

export interface PrivatePluginContextResolveOptions
  extends PluginContextResolveOptions {
  [SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF]?: symbol
}

export class PluginContext {
  debug: LoggingFunction
  info: LoggingFunction
  warn: LoggingFunction
  error: (error: RollupError | string) => never
  resolve: (
    source: string,
    importer?: string,
    options?: PluginContextResolveOptions,
  ) => Promise<ResolvedId | null>
  emitFile: (file: EmittedAsset) => string
  getFileName: (referenceId: string) => string
  getModuleInfo: (id: string) => ModuleInfo | null
  getModuleIds: () => IterableIterator<string>
  addWatchFile: (id: string) => void
  /**
   * @deprecated This rollup API won't be supported by rolldown. Using this API will cause runtime error.
   */
  parse: (input: string, options?: any) => any

  constructor(
    options: NormalizedInputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
  ) {
    const onLog = options.onLog
    const pluginName = plugin.name || 'unknown'
    const logLevel = options.logLevel
    this.debug = getLogHandler(
      LOG_LEVEL_DEBUG,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    )
    this.warn = getLogHandler(
      LOG_LEVEL_WARN,
      'PLUGIN_WARNING',
      onLog,
      pluginName,
      logLevel,
    )
    this.info = getLogHandler(
      LOG_LEVEL_INFO,
      'PLUGIN_LOG',
      onLog,
      pluginName,
      logLevel,
    )
    this.error = (e): never => {
      return error(logPluginError(normalizeLog(e), pluginName))
    }
    this.resolve = async (source, importer, options) => {
      let receipt: number | undefined = undefined
      if (options != null) {
        receipt = data.saveResolveOptions(options)
      }
      const res = await context.resolve(source, importer, {
        custom: receipt,
        skipSelf: options?.skipSelf,
      })
      if (receipt != null) {
        data.removeSavedResolveOptions(receipt)
      }

      if (res == null) return null
      const info = data.getModuleOption(res.id) || ({} as ModuleOptions)
      return { ...res, ...info }
    }
    this.emitFile = (file: EmittedAsset): string => {
      if (file.type !== 'asset') {
        return unimplemented(
          'PluginContext.emitFile: only asset type is supported',
        )
      }
      return context.emitFile({
        ...file,
        originalFileName: file.originalFileName || undefined,
        source: bindingAssetSource(file.source),
      })
    }
    this.getFileName = context.getFileName.bind(context)
    this.getModuleInfo = (id: string) => data.getModuleInfo(id, context)
    this.getModuleIds = () => data.getModuleIds(context)
    this.parse = unsupported(
      '`PluginContext#parse` is not supported by rolldown.',
    )
    this.addWatchFile = () => {}
  }
}
