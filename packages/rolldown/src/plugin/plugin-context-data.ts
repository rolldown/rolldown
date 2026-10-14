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
import {
  type DroppableBox,
  releaseOrDefer,
  shouldEagerlyFreeOutputs,
} from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import type { RenderedChunkMeta } from '.';
import type { PluginContextResolveOptions } from './plugin-context';

// A JavaScript context built on a retained box: `PluginContextImpl` or
// `ChunkingContextImpl`.
interface BindingContextOwner {
  releaseBinding(): void;
}

type ReleaseFailure = { error: unknown } | undefined;

// Releases every item even when one throws, so one failure cannot leave the
// rest attached; keeps the first error for the caller to rethrow once the
// whole release is done.
function releaseEach<T>(
  items: Iterable<T>,
  release: (item: T) => void,
  failure: ReleaseFailure,
): ReleaseFailure {
  for (const item of items) {
    try {
      release(item);
    } catch (error) {
      failure ??= { error };
    }
  }
  return failure;
}

export class PluginContextData {
  moduleOptionMap: Map<string, ModuleOptions> = new Map();
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map();
  loadModulePromiseMap: Map<string, Promise<void>> = new Map();
  renderedChunkMeta: RenderedChunkMeta | null = null;
  normalizedInputOptions: NormalizedInputOptionsImpl | null = null;
  normalizedOutputOptions: NormalizedOutputOptionsImpl | null = null;

  // Native option boxes the cached wrappers above read from. A threadless WASI
  // host may never run GC finalizers, so these are released explicitly.
  #retainedOptionBoxes: Set<BindingNormalizedOptions> = new Set();

  // Context boxes kept past their own hook: Rollup lets a plugin keep using a
  // build-scoped context after the hook settles (Vite's
  // `vite:watch-package-data` calls a `buildStart`-bound `this.addWatchFile`
  // from `resolveId`). Drained only by `releaseRetainedOptionBoxes`, never by
  // `clear()`, which runs before `writeBundle`.
  #retainedContextBoxes: Set<DroppableBox> = new Set();
  // The JavaScript contexts built on those boxes, detached at the same point.
  #retainedContexts: Set<BindingContextOwner> = new Set();

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
      // source, and a threadless WASI host may never run GC finalizers, so
      // hand out a plain-data snapshot and release the box immediately.
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

  // Each hook call marshals a fresh options box, but only the one behind the
  // cached wrapper is read: keep it and drop the duplicates. renderStart
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

  // ONLY for boxes minted during the build: the registry drains when the
  // `generate()`/`write()` call settles, so an output- or close-side box parked
  // here would never be released. Pass the JavaScript context built on the box
  // so the drain detaches it too. No-op outside threadless WASI.
  retainContextBox(box: DroppableBox, context: BindingContextOwner): void {
    if (shouldEagerlyFreeOutputs()) {
      this.#retainedContextBoxes.add(box);
      this.#retainedContexts.add(context);
    }
  }

  // The native `invalidateJsSideCache` callback, fired at the end of
  // `bundle_up`, i.e. before `writeBundle`: leave the context boxes alone (a
  // `buildStart`-bound `this.addWatchFile` may run in `writeBundle`). Option
  // boxes are safe to release because their cached wrappers keep serving reads.
  clear(): void {
    this.renderedChunkMeta = null;
    this.loadModulePromiseMap.clear();
    this.#releaseOptionBoxes();
  }

  // Terminal release once a build, `scan()` or watcher close settles. The only
  // drain for context boxes; also covers option boxes when the invalidate
  // callback never fired (failed builds, `scan()`). Idempotent; no-op off
  // threadless WASI. Both parts run to the end before the first error, if
  // any, is rethrown.
  releaseRetainedOptionBoxes(): void {
    let failure: ReleaseFailure;
    try {
      this.#releaseContextBoxes();
    } catch (error) {
      failure = { error };
    }
    try {
      this.#releaseOptionBoxes();
    } catch (error) {
      failure ??= { error };
    }
    if (failure) {
      throw failure.error;
    }
  }

  // `releaseOrDefer`, not `dropInner()`: a fire-and-forget `this.load()` /
  // `this.resolve()` may still hold a shared borrow on its context, and an
  // exclusive drop would throw (see `utils/threadless-free.ts`).
  //
  // A context a plugin or a `codeSplitting` classifier keeps would otherwise
  // keep its binding wrapper, whose methods keep the binding instance alive (on
  // workerd, a disposed instance's whole memory), so the contexts are detached
  // as well. A call still in flight holds the box itself, so detaching cannot
  // cut a deferred drop short. See
  // internal-docs/workerd-managed-instance/implementation.md.
  #releaseContextBoxes(): void {
    if (!shouldEagerlyFreeOutputs()) {
      return;
    }
    let failure = releaseEach(
      this.#retainedContexts,
      (context) => context.releaseBinding(),
      undefined,
    );
    this.#retainedContexts.clear();
    failure = releaseEach(this.#retainedContextBoxes, releaseOrDefer, failure);
    this.#retainedContextBoxes.clear();
    if (failure) {
      throw failure.error;
    }
  }

  // Copy box-backed values into the cached wrappers, release the boxes, then
  // detach the wrappers from their binding objects, whose methods keep the
  // binding instance alive while a plugin keeps the options. Fields backed by
  // the user's `outputOptions` stay lazy: user accessors must not run, and
  // maybe throw, from a cleanup path.
  #releaseOptionBoxes(): void {
    if (!shouldEagerlyFreeOutputs() || this.#retainedOptionBoxes.size === 0) {
      return;
    }
    const wrappers = [this.normalizedInputOptions, this.normalizedOutputOptions];
    for (const wrapper of wrappers) {
      if (wrapper == null) continue;
      try {
        wrapper.materializeBoxBackedFields();
      } catch {
        // Later reads of the affected fields then throw the wrapper's
        // "already released" error; cleanup must not throw or stop.
      }
    }
    let failure = releaseEach(this.#retainedOptionBoxes, (box) => box.dropInner(), undefined);
    this.#retainedOptionBoxes.clear();
    failure = releaseEach(wrappers, (wrapper) => wrapper?.releaseBinding(), failure);
    if (failure) {
      throw failure.error;
    }
  }
}
