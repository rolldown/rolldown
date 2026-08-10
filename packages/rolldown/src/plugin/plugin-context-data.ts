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
import { shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import type { RenderedChunkMeta } from '.';
import type { PluginContextResolveOptions } from './plugin-context';

export class PluginContextData {
  moduleOptionMap: Map<string, ModuleOptions> = new Map();
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map();
  loadModulePromiseMap: Map<string, Promise<void>> = new Map();
  renderedChunkMeta: RenderedChunkMeta | null = null;
  normalizedInputOptions: NormalizedInputOptionsImpl | null = null;
  normalizedOutputOptions: NormalizedOutputOptionsImpl | null = null;

  // Native option boxes the cached wrappers above still read from. On the
  // threadless-WASI flavor GC finalizers never run, so these are released
  // explicitly (see `#releaseOptionBoxes`); every other box a hook marshals is
  // a never-read duplicate and is dropped on arrival.
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
      // source, and the threadless flavor never runs GC finalizers, so hand
      // out a plain-data snapshot and release the box immediately.
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
  // box, but only the box behind the first (cached) wrapper is ever read: keep
  // that one for `clear()` and drop the duplicates on the spot. renderStart
  // passes the SAME box to both getters, hence the membership check below.
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

  // Terminal release for builds where the native invalidate callback never
  // fires: it only runs after a successful generate (bundle.rs `bundle_up`,
  // between generateBundle and writeBundle), so failed builds, scan(), and
  // writeBundle would otherwise strand their retained boxes. No-op on native
  // flavors and idempotent, so every terminal path may call it repeatedly.
  releaseRetainedOptionBoxes(): void {
    this.#releaseOptionBoxes();
  }

  // Settle point for the cached options wrappers on the threadless-WASI
  // flavor: copy every box-backed value to JavaScript, then release the boxes.
  // The wrappers stay cached, so later hooks and user-held references keep
  // reading them. Only BOX-BACKED data is materialized — fields backed by the
  // user's original `outputOptions` stay lazy, because running user accessors
  // from a cleanup path must never turn a successful build into a rejection.
  // The whole release is best-effort: one failure must not strand the rest.
  #releaseOptionBoxes(): void {
    if (!shouldEagerlyFreeOutputs() || this.#retainedOptionBoxes.size === 0) {
      return;
    }
    for (const wrapper of [this.normalizedInputOptions, this.normalizedOutputOptions]) {
      if (wrapper == null) continue;
      try {
        wrapper.materializeBoxBackedFields();
      } catch {
        // Later reads of the affected fields then report the documented
        // "memory has been freed" error; cleanup must not throw or stop.
      }
    }
    for (const box of this.#retainedOptionBoxes) {
      try {
        box.dropInner();
      } catch {
        // Same best-effort contract as above.
      }
    }
    this.#retainedOptionBoxes.clear();
  }
}
