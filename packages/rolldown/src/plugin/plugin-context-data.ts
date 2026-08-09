import type {
  ModuleOptions,
  NormalizedInputOptions,
  NormalizedOutputOptions,
  OutputOptions,
  RolldownPlugin,
} from '..';
import type { BindingPluginContext } from '../binding.cjs';
import { type BindingNormalizedOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { NormalizedInputOptionsImpl } from '../options/normalized-input-options';
import { NormalizedOutputOptionsImpl } from '../options/normalized-output-options';
import type { ModuleInfo } from '../types/module-info';
import { getLazyFields } from '../types/plain-object-like';
import { shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import type { RenderedChunkMeta } from '.';
import type { PluginContextResolveOptions } from './plugin-context';

export class PluginContextData {
  moduleOptionMap: Map<string, ModuleOptions> = new Map();
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map();
  loadModulePromiseMap: Map<string, Promise<void>> = new Map();
  renderedChunkMeta: RenderedChunkMeta | null = null;
  normalizedInputOptions: NormalizedInputOptions | null = null;
  normalizedOutputOptions: NormalizedOutputOptions | null = null;

  // The native option boxes the cached wrappers above still read from, kept
  // so `clear()` can snapshot the wrappers and release the boxes on the
  // threadless-WASI flavor (see `#releaseOptionBoxes`). Every other option
  // box a hook invocation marshals is a never-read duplicate of these and is
  // dropped on arrival there.
  #retainedOptionBoxes: Set<BindingNormalizedOptions> = new Set();

  constructor(
    private onLog: LogHandler,
    private outputOptions: OutputOptions,
    private normalizedInputPlugins: RolldownPlugin[],
    private normalizedOutputPlugins: RolldownPlugin[],
  ) {}

  updateModuleOption(id: string, option: ModuleOptions): ModuleOptions {
    const existing = this.moduleOptionMap.get(id);
    if (existing) {
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
      this.moduleOptionMap.set(id, option);
      return option;
    }
    return existing;
  }

  getModuleOption(id: string): ModuleOptions {
    const option = this.moduleOptionMap.get(id);
    if (!option) {
      const raw: ModuleOptions = {
        moduleSideEffects: null,
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
      // Each call mints a fresh module-info box retaining the module's full
      // source; on the threadless flavor GC finalizers (its normal
      // reclamation path) never run, so hand out a plain-data snapshot and
      // release the box immediately.
      const info = shouldEagerlyFreeOutputs()
        ? snapshotModuleInfo(bindingInfo, this.getModuleOption(id))
        : transformModuleInfo(bindingInfo, this.getModuleOption(id));
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
      set: (v: any) => {
        this.updateModuleOption(id, {
          moduleSideEffects: v,
          meta: info.meta,
          invalidate: true,
        });
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
    if (this.normalizedInputOptions == null) {
      this.normalizedInputOptions = new NormalizedInputOptionsImpl(
        opts,
        this.onLog,
        this.normalizedInputPlugins,
      );
      this.#trackOptionBox(opts);
    } else {
      this.#dropDuplicateOptionBox(opts);
    }
    return this.normalizedInputOptions;
  }

  getOutputOptions(opts: BindingNormalizedOptions): NormalizedOutputOptions {
    if (this.normalizedOutputOptions == null) {
      this.normalizedOutputOptions = new NormalizedOutputOptionsImpl(
        opts,
        this.outputOptions,
        this.normalizedOutputPlugins,
      );
      this.#trackOptionBox(opts);
    } else {
      this.#dropDuplicateOptionBox(opts);
    }
    return this.normalizedOutputOptions;
  }

  // Every hook invocation marshals its own fresh `BindingNormalizedOptions`
  // box, but only the box behind the first (cached) wrapper is ever read. On
  // the threadless-WASI flavor GC finalizers (the boxes' normal reclamation
  // path) never run, so remember the read box for `clear()` and release every
  // never-read duplicate on the spot. renderStart passes the SAME box to both
  // getters, hence the set membership check before dropping.
  #trackOptionBox(opts: BindingNormalizedOptions): void {
    if (shouldEagerlyFreeOutputs()) {
      this.#retainedOptionBoxes.add(opts);
    }
  }

  #dropDuplicateOptionBox(opts: BindingNormalizedOptions): void {
    if (shouldEagerlyFreeOutputs() && !this.#retainedOptionBoxes.has(opts)) {
      opts.dropInner();
    }
  }

  clear(): void {
    this.renderedChunkMeta = null;
    this.loadModulePromiseMap.clear();
    this.#releaseOptionBoxes();
  }

  // The native side invalidates the JS caches once per completed generate
  // (before writeBundle). On the threadless-WASI flavor that is also the
  // settle point for the cached options wrappers: copy their remaining lazy
  // fields to JavaScript and release the native boxes behind them. The
  // wrappers stay cached as plain data, so later hooks — and user code that
  // kept a reference — read the snapshot while each fresh incoming box is
  // dropped as a never-read duplicate.
  #releaseOptionBoxes(): void {
    if (!shouldEagerlyFreeOutputs() || this.#retainedOptionBoxes.size === 0) {
      return;
    }
    for (const wrapper of [this.normalizedInputOptions, this.normalizedOutputOptions]) {
      if (wrapper == null) continue;
      for (const field of getLazyFields(wrapper)) {
        // property access is enough to evaluate and cache the lazy field
        const _value = (wrapper as any)[field];
      }
    }
    for (const box of this.#retainedOptionBoxes) {
      box.dropInner();
    }
    this.#retainedOptionBoxes.clear();
  }
}
