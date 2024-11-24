import type { BindingPluginContext } from '../binding'
import type {
  CustomPluginOptions,
  ModuleOptions,
  Plugin,
  ResolvedId,
} from './index'
import { MinimalPluginContext } from '../plugin/minimal-plugin-context'
import { AssetSource, bindingAssetSource } from '../utils/asset-source'
import { unimplemented, unsupported } from '../utils/misc'
import { ModuleInfo } from '../types/module-info'
import { PluginContextData } from './plugin-context-data'
import { SYMBOL_FOR_RESOLVE_CALLER_THAT_SKIP_SELF } from '../constants/plugin-context'
import { PartialNull } from '../types/utils'
import { bindingifySideEffects } from '../utils/transform-side-effects'
import type { LogHandler, LogLevelOption } from '../rollup'

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
  readonly load: (
    options: { id: string; resolveDependencies?: boolean } & Partial<
      PartialNull<ModuleOptions>
    >,
  ) => Promise<ModuleInfo>
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
    context: BindingPluginContext,
    plugin: Plugin,
    data: PluginContextData,
    onLog: LogHandler,
    logLevel: LogLevelOption,
  ) {
    super(onLog, logLevel, plugin)
    this.load = async ({ id, ...options }) => {
      // resolveDependencies always true at rolldown
      const moduleInfo = data.getModuleInfo(id, context)
      if (moduleInfo && moduleInfo.code !== null /* module already parsed */) {
        return moduleInfo
      }
      const rawOptions = {
        meta: options.meta || {},
        moduleSideEffects: options.moduleSideEffects || null,
      }
      data.updateModuleOption(id, rawOptions)

      async function createLoadModulePromise() {
        const loadPromise = data.loadModulePromiseMap.get(id)
        if (loadPromise) {
          return loadPromise
        }
        let resolveFn
        // TODO: If is not resolved, we need to set a time to avoid waiting.
        const promise = new Promise<void>((resolve, _) => {
          resolveFn = resolve
        })
        data.loadModulePromiseMap.set(id, promise)
        try {
          await context.load(
            id,
            bindingifySideEffects(options.moduleSideEffects),
            resolveFn!,
          )
        } finally {
          // If the load module has failed, avoid it re-load using unresolved promise.
          data.loadModulePromiseMap.delete(id)
        }
        return promise
      }

      // Here using one promise to avoid pass more callback to rust side, it only accept one callback, other will be ignored.
      await createLoadModulePromise()
      return data.getModuleInfo(id, context)!
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
    this.addWatchFile = context.addWatchFile.bind(context)
  }
}
