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
import type { LogHandler, LogLevelOption } from '../types/misc'
import { LOG_LEVEL_WARN } from '../log/logging'
import { logCycleLoading } from '../log/logs'

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
  constructor(
    private context: BindingPluginContext,
    plugin: Plugin,
    private data: PluginContextData,
    private onLog: LogHandler,
    logLevel: LogLevelOption,
    private currentLoadingModule?: string,
  ) {
    super(onLog, logLevel, plugin.name!)
  }

  public async load(
    options: { id: string; resolveDependencies?: boolean } & Partial<
      PartialNull<ModuleOptions>
    >,
  ): Promise<ModuleInfo> {
    const id = options.id
    if (id === this.currentLoadingModule) {
      this.onLog(
        LOG_LEVEL_WARN,
        logCycleLoading(this.pluginName, this.currentLoadingModule),
      )
    }
    // resolveDependencies always true at rolldown
    const moduleInfo = this.data.getModuleInfo(id, this.context)
    if (moduleInfo && moduleInfo.code !== null /* module already parsed */) {
      return moduleInfo
    }
    const rawOptions = {
      meta: options.meta || {},
      moduleSideEffects: options.moduleSideEffects || null,
    }
    this.data.updateModuleOption(id, rawOptions)

    async function createLoadModulePromise(
      context: BindingPluginContext,
      data: PluginContextData,
    ) {
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
    await createLoadModulePromise(this.context, this.data)
    return this.data.getModuleInfo(id, this.context)!
  }

  public async resolve(
    source: string,
    importer?: string,
    options?: PluginContextResolveOptions,
  ): Promise<ResolvedId | null> {
    let receipt: number | undefined = undefined
    if (options != null) {
      receipt = this.data.saveResolveOptions(options)
    }
    const res = await this.context.resolve(source, importer, {
      custom: receipt,
      skipSelf: options?.skipSelf,
    })
    if (receipt != null) {
      this.data.removeSavedResolveOptions(receipt)
    }

    if (res == null) return null
    const info = this.data.getModuleOption(res.id) || ({} as ModuleOptions)
    return { ...res, ...info }
  }

  public emitFile(file: EmittedAsset): string {
    if (file.type !== 'asset') {
      return unimplemented(
        'PluginContext.emitFile: only asset type is supported',
      )
    }
    return this.context.emitFile({
      ...file,
      originalFileName: file.originalFileName || undefined,
      source: bindingAssetSource(file.source),
    })
  }

  public getFileName(referenceId: string): string {
    return this.context.getFileName(referenceId)
  }

  public getModuleInfo(id: string): ModuleInfo | null {
    return this.data.getModuleInfo(id, this.context)
  }

  public getModuleIds(): IterableIterator<string> {
    return this.data.getModuleIds(this.context)
  }

  public addWatchFile(id: string): void {
    this.context.addWatchFile(id)
  }

  /**
   * @deprecated This rollup API won't be supported by rolldown. Using this API will cause runtime error.
   */
  public parse(_input: string, _options?: any): any {
    unsupported('`PluginContext#parse` is not supported by rolldown.')
  }
}
