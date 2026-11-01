import type {
  ModuleOptions,
  NormalizedInputOptions,
  NormalizedOutputOptions,
  OutputOptions,
  RolldownPlugin,
} from '..';
import type { BindingOutputChunk, BindingPluginContext } from '../binding.cjs';
import { type BindingNormalizedOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { NormalizedInputOptionsImpl } from '../options/normalized-input-options';
import { NormalizedOutputOptionsImpl } from '../options/normalized-output-options';
import type { ModuleInfo } from '../types/module-info';
import { transformModuleInfo } from '../utils/transform-module-info';
import type { RenderedChunkMeta } from '.';
import type { PluginContextResolveOptions } from './plugin-context';

/**
 * This entry keeps the `chunk.code` string of one output chunk. All plugins of a build use this
 * one string. Each call of a `generateBundle` or `writeBundle` hook gets new `BindingOutputChunk`
 * objects. Without this cache, each plugin that reads `chunk.code` copies the code from Rust again.
 */
export interface OutputCodeCacheEntry {
  /**
   * This is the `BindingOutputChunk.getCodeIdentity()` value of the chunk data that `code` came
   * from. The identity is the address of that chunk data in Rust memory. The `chunk.code` getter in
   * `transformToMutableRollupOutputChunk` compares only the identity, never the code. When the
   * identity is the same, the getter returns `code`. Otherwise, the getter copies the code again
   * and replaces this entry.
   */
  identity: number;
  /**
   * This reference to the chunk data keeps `identity` correct while this entry exists:
   * - Rust does not free the chunk data, so new chunk data cannot get the same address.
   * - `update_output_chunk` (`crates/rolldown_binding/src/types/binding_output_chunk.rs:163`)
   *   cannot write a plugin change into this chunk data, because `Arc::get_mut` fails while this
   *   second reference exists. The function writes the change into new chunk data at a new
   *   address. So the getter sees a new identity and copies the new code. It does not return the
   *   old `code`.
   */
  holder: BindingOutputChunk;
  code: string;
}

/**
 * `undefined` means no hook or plugin write has set `moduleSideEffects`. Readers then
 * use the effective native value. `null` is an explicit write and stays `null`.
 * The public `ModuleOptions` type stays `boolean | 'no-treeshake' | null` so hook
 * result types keep intersecting.
 */
type StoredModuleOptions = Omit<ModuleOptions, 'moduleSideEffects'> & {
  moduleSideEffects: ModuleOptions['moduleSideEffects'] | undefined;
};

export class PluginContextData {
  moduleOptionMap: Map<string, StoredModuleOptions> = new Map();
  /**
   * The cache is `null` in parallel plugin workers. In a worker, each plugin has its own
   * `PluginContextData`, so the cache never finds a match. See `parallel-plugin-worker.ts`.
   */
  outputCodeCache: Map<string, OutputCodeCacheEntry> | null;
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map();
  loadModulePromiseMap: Map<string, Promise<void>> = new Map();
  renderedChunkMeta: RenderedChunkMeta | null = null;
  normalizedInputOptions: NormalizedInputOptions | null = null;
  normalizedOutputOptions: NormalizedOutputOptions | null = null;

  constructor(
    private onLog: LogHandler,
    private outputOptions: OutputOptions,
    private normalizedInputPlugins: RolldownPlugin[],
    private normalizedOutputPlugins: RolldownPlugin[],
    cacheOutputCode = true,
  ) {
    this.outputCodeCache = cacheOutputCode ? new Map() : null;
  }

  updateModuleOption(id: string, option: ModuleOptions): StoredModuleOptions {
    const existing = this.moduleOptionMap.get(id);
    if (existing) {
      // `null` from a hook means "not specified" and must not clear a previous value.
      // An explicit `null` write assigns the stored field directly.
      if (option.moduleSideEffects != null) {
        existing.moduleSideEffects = option.moduleSideEffects;
      }
      if (option.meta != null) {
        Object.assign(existing.meta, option.meta);
      }
      if (option.invalidate != null) {
        existing.invalidate = option.invalidate;
      }
    } else {
      const stored: StoredModuleOptions = {
        ...option,
        moduleSideEffects: option.moduleSideEffects == null ? undefined : option.moduleSideEffects,
      };
      this.moduleOptionMap.set(id, stored);
      return stored;
    }
    return existing;
  }

  getModuleOption(id: string): StoredModuleOptions {
    const option = this.moduleOptionMap.get(id);
    if (!option) {
      const raw: StoredModuleOptions = {
        moduleSideEffects: undefined,
        meta: {},
      };
      this.moduleOptionMap.set(id, raw);
      return raw;
    }
    return option;
  }

  getModuleInfo(id: string, context: BindingPluginContext): ModuleInfo | null {
    const bindingInfo = context.getModuleInfo(id);
    if (bindingInfo) {
      const info = transformModuleInfo(bindingInfo, this.getModuleOption(id));
      return this.proxyModuleInfo(id, info);
    }
    return null;
  }

  proxyModuleInfo(id: string, info: ModuleInfo): ModuleInfo {
    let moduleSideEffects = info.moduleSideEffects;
    Object.defineProperty(info, 'moduleSideEffects', {
      get() {
        return moduleSideEffects;
      },
      set: (v: ModuleInfo['moduleSideEffects']) => {
        const stored = this.getModuleOption(id);
        stored.moduleSideEffects = v;
        stored.invalidate = true;
        moduleSideEffects = v;
      },
    });
    return info;
  }

  getModuleIds(context: BindingPluginContext): ArrayIterator<string> {
    const moduleIds = context.getModuleIds();
    return moduleIds.values();
  }

  saveResolveOptions(options: PluginContextResolveOptions): number {
    const index = this.resolveOptionsMap.size;
    this.resolveOptionsMap.set(index, options);
    return index;
  }

  getSavedResolveOptions(receipt: number): PluginContextResolveOptions | undefined {
    return this.resolveOptionsMap.get(receipt);
  }

  removeSavedResolveOptions(receipt: number): void {
    this.resolveOptionsMap.delete(receipt);
  }

  setRenderChunkMeta(meta: RenderedChunkMeta): void {
    this.renderedChunkMeta = meta;
  }

  getRenderChunkMeta(): RenderedChunkMeta | null {
    return this.renderedChunkMeta;
  }

  getInputOptions(opts: BindingNormalizedOptions): NormalizedInputOptions {
    this.normalizedInputOptions ??= new NormalizedInputOptionsImpl(
      opts,
      this.onLog,
      this.normalizedInputPlugins,
    );
    return this.normalizedInputOptions;
  }

  getOutputOptions(opts: BindingNormalizedOptions): NormalizedOutputOptions {
    this.normalizedOutputOptions ??= new NormalizedOutputOptionsImpl(
      opts,
      this.outputOptions,
      this.normalizedOutputPlugins,
    );
    return this.normalizedOutputOptions;
  }

  clear(): void {
    this.renderedChunkMeta = null;
    this.loadModulePromiseMap.clear();
    // Watch mode reuses this object across rebuilds; entries hold the previous build's chunk data.
    this.outputCodeCache?.clear();
  }
}
