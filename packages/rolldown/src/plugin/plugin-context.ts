import type { BindingPluginContext } from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type {
  CustomPluginOptions,
  ModuleOptions,
  Plugin,
  ResolvedId,
} from './index'
import { MinimalPluginContext } from '../log/logger'
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

export class PluginContext extends MinimalPluginContext {
  readonly resolve: (
    source: string,
    importer?: string,
    options?: PluginContextResolveOptions,
  ) => Promise<ResolvedId | null>
  readonly emitFile: (file: EmittedAsset) => string
  readonly getFileName: (referenceId: string) => string
  readonly getModuleInfo: (id: string) => ModuleInfo | null
  readonly getModuleIds: () => IterableIterator<string>
  readonly addWatchFile: (id: string) => void
  /**
   * @deprecated This rollup API won't be supported by rolldown. Using this API will cause runtime error.
   */
  readonly parse: (input: string, options?: any) => any

  constructor(
    options: NormalizedInputOptions,
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
  ) {
    super(options, plugin)
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
    this.addWatchFile = context.addWatchFile.bind(context)
  }
}
