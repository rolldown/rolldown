import type { RollupError, LoggingFunction } from '../rollup'
import type { BindingPluginContext } from '../binding'
import { getLogHandler, normalizeLog } from '../log/logHandler'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { Plugin } from './index'
import { LOG_LEVEL_DEBUG, LOG_LEVEL_INFO, LOG_LEVEL_WARN } from '../log/logging'
import { error, logPluginError } from '../log/logs'
import { AssetSource, bindingAssetSource } from '../utils/asset-source'
import { unimplemented, unsupported } from '../utils/misc'
import { ModuleInfo } from '../types/module-info'
import { transformModuleInfo } from '../utils'

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
  resolve: BindingPluginContext['resolve']
  emitFile: (file: EmittedAsset) => string
  getFileName: (referenceId: string) => string
  getModuleInfo: (id: string) => ModuleInfo | null
  getModuleIds: () => IterableIterator<string>
  /**
   * @deprecated This rollup API won't be supported by rolldown. Using this API will cause runtime error.
   */
  parse: (input: string, options?: any) => any

  modules = new Map<string, ModuleInfo>()
  moduleIds: Array<string> | null = null
  constructor(
    options: NormalizedInputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
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
    this.resolve = context.resolve.bind(context)
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
    this.getModuleInfo = (id: string) => {
      if (this.modules.has(id)) {
        return this.modules.get(id) ?? null
      }
      const bindingInfo = context.getModuleInfo(id)
      if (bindingInfo) {
        const info = transformModuleInfo(bindingInfo)
        this.modules.set(id, info)
        return info
      }
      return null
    }
    this.getModuleIds = () => {
      if (this.moduleIds) {
        return this.moduleIds.values()
      }
      const moduleIds = context.getModuleIds()
      if (moduleIds) {
        this.moduleIds = moduleIds
        return moduleIds.values()
      }
      return [].values()
    }
    this.parse = unsupported(
      '`PluginContext#parse` is not supported by rolldown.',
    )
  }
}
