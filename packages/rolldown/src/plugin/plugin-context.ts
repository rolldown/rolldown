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

export interface EmittedAsset {
  type: 'asset'
  name?: string
  fileName?: string
  source: AssetSource
}

export type EmittedFile = EmittedAsset

export class PluginContext {
  debug: LoggingFunction
  info: LoggingFunction
  warn: LoggingFunction
  error: (error: RollupError | string) => never
  resolve: (
    source: string,
    importer?: string,
    options?: {
      // assertions?: Record<string, string>
      custom?: CustomPluginOptions
      // isEntry?: boolean
      skipSelf?: boolean
    },
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
    this.resolve = async (
      source: string,
      importer?: string,
      options?: {
        // assertions?: Record<string, string>
        custom?: CustomPluginOptions
        // isEntry?: boolean
        skipSelf?: boolean
      },
    ) => {
      let custom = options?.custom && data.setResolveCustom(options.custom)
      const res = await context.resolve(source, importer, {
        custom,
        skipSelf: options?.skipSelf,
      })
      typeof custom === 'number' && data.removeResolveCustom(custom)
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
