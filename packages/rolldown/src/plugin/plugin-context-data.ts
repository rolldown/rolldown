import type {
  CustomPluginOptions,
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
import { parseModuleId, stringifyModuleId } from '../utils/module-id';
import {
  type DroppableBox,
  releaseOrDefer,
  shouldEagerlyFreeOutputs,
} from '../utils/threadless-free';
import {
  type ModuleSideEffectsAccess,
  type ModuleSideEffectsOptionFn,
  snapshotModuleInfo,
  transformModuleInfo,
} from '../utils/transform-module-info';
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
 * Returns `next`, or the first receipt after it that no map in `maps` uses. The counter wraps at
 * 2^32 so the receipt stays a valid `u32`. After a wrap, receipts that are still in use are
 * skipped.
 */
function freeReceipt(next: number, ...maps: Map<number, unknown>[]): number {
  let receipt = next;
  while (maps.some((map) => map.has(receipt))) {
    receipt = (receipt + 1) >>> 0;
  }
  return receipt;
}

/**
 * The module options that JS keeps for one module. `moduleSideEffects` is not here: only Rust
 * keeps it. See internal-docs/cache/implementation.md.
 */
export type JsModuleOptions = Pick<ModuleOptions, 'meta'>;

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
  moduleOptionMap: Map<string, JsModuleOptions> = new Map();
  /** The module options that each module started with. See `createModuleOptions`. */
  private resolvedModuleOptionMap: Map<string, JsModuleOptions> = new Map();
  /**
   * The `meta` of each JS resolution: a `resolveId` or `resolveDynamicImport` object result, or a
   * `this.load` call. See `saveResolutionOptions`.
   */
  private resolutionOptionsMap: Map<number, JsModuleOptions> = new Map();
  /**
   * The resolutions saved before the last partial scan started. The next partial scan drops them.
   * See `resetModuleOptions`.
   */
  private olderResolutionOptionsMap: Map<number, JsModuleOptions> = new Map();
  private nextResolutionOptionsReceipt = 0;
  outputCodeCache: Map<string, OutputCodeCacheEntry> = new Map();
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map();
  /** The next `this.resolve` receipt. See `saveResolveOptions`. */
  private nextResolveOptionsReceipt = 0;
  loadModulePromiseMap: Map<string, Promise<void>> = new Map();
  loadModuleWithDependenciesPromiseMap: Map<string, Promise<void>> = new Map();
  renderedChunkMeta: RenderedChunkMeta | null = null;
  normalizedInputOptions: NormalizedInputOptionsImpl | null = null;
  normalizedOutputOptions: NormalizedOutputOptionsImpl | null = null;
  /** `experimental.importAttributes` */
  importAttributes = false;
  /**
   * `treeshake.moduleSideEffects` when it is a function. The `moduleSideEffects` getter and setter
   * pass it to Rust, which calls it only when the value needs it.
   */
  moduleSideEffectsOption: ModuleSideEffectsOptionFn | undefined = undefined;

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
  // One build-scoped context box for live `ModuleInfo.moduleSideEffects`
  // reads/writes across hooks on threadless WASI, so a `ModuleInfo` retained
  // from an earlier hook (e.g. `load`) stays live through `buildEnd` without
  // retaining per-module boxes or pinning a disposed Wasm instance past the build.
  #sideEffectsBox: (ModuleSideEffectsAccess & DroppableBox) | undefined = undefined;
  #released = false;
  readonly #sideEffectsAccess: ModuleSideEffectsAccess = {
    getModuleSideEffects: (id, callOption) => {
      const box = this.#sideEffectsBox;
      if (box === undefined) {
        throw new Error('This module info has already released its binding object');
      }
      return box.getModuleSideEffects(id, callOption);
    },
    setModuleSideEffects: (id, value, callOption) => {
      const box = this.#sideEffectsBox;
      if (box === undefined) {
        throw new Error('This module info has already released its binding object');
      }
      return box.setModuleSideEffects(id, value, callOption);
    },
  };

  constructor(
    private onLog: LogHandler,
    private outputOptions: OutputOptions,
    private normalizedInputPlugins: RolldownPlugin[],
    private normalizedOutputPlugins: RolldownPlugin[],
  ) {}

  /**
   * The module id of `rawId` imported with `attributes`, as JS keys it. Without
   * `experimental.importAttributes`, it is always `rawId`.
   */
  toModuleId(rawId: string, attributes: Record<string, string> | undefined): string {
    return this.importAttributes ? stringifyModuleId(rawId, attributes) : rawId;
  }

  /**
   * Without `experimental.importAttributes`, plugins see no attributes, so the module options of
   * an external are keyed by its raw id, even though its module id has the attributes.
   */
  private moduleOptionKey(id: string): string {
    return this.importAttributes ? id : parseModuleId(id).rawId;
  }

  /** Merges the `meta` of a `load` or `transform` result into the `meta` of the module. */
  updateModuleMeta(id: string, meta: CustomPluginOptions): void {
    const key = this.moduleOptionKey(id);
    const existing = this.moduleOptionMap.get(key);
    if (existing) {
      Object.assign(existing.meta, meta);
    } else {
      // Copy `meta` as Rollup does: merges change it in place, and a plugin may return one
      // object as the meta of several modules.
      this.moduleOptionMap.set(key, { meta: { ...meta } });
    }
  }

  /**
   * Saves the options of one resolution and returns its receipt. The receipt goes to Rust with the
   * resolution. Only the resolution that creates a module sets the module's start options, as in
   * Rollup: Rust calls `createModuleOptions` with the receipt of that resolution.
   */
  saveResolutionOptions(options: JsModuleOptions): number {
    const receipt = freeReceipt(
      this.nextResolutionOptionsReceipt,
      this.resolutionOptionsMap,
      this.olderResolutionOptionsMap,
    );
    this.nextResolutionOptionsReceipt = (receipt + 1) >>> 0;
    this.resolutionOptionsMap.set(receipt, options);
    return receipt;
  }

  takeResolutionOptions(receipt: number | null | undefined): JsModuleOptions | undefined {
    if (receipt == null) return undefined;
    for (const map of [this.resolutionOptionsMap, this.olderResolutionOptionsMap]) {
      const options = map.get(receipt);
      if (options) {
        map.delete(receipt);
        return options;
      }
    }
    return undefined;
  }

  /**
   * Rust created the module `id` from the resolution with `receipt`, and its `load` hook has not
   * run yet. The module starts with the options of that resolution, and a partial scan resets the
   * module to them (`resetModuleOptions`). See internal-docs/cache/implementation.md.
   */
  createModuleOptions(id: string, receipt: number | null): void {
    const key = this.moduleOptionKey(id);
    const resolution = this.takeResolutionOptions(receipt);
    // Copy `meta` as Rollup does: merges change it in place, and a plugin may return one object as
    // the meta of several modules.
    this.moduleOptionMap.set(key, { meta: { ...resolution?.meta } });
    this.resolvedModuleOptionMap.set(key, { meta: { ...resolution?.meta } });
  }

  getModuleOption(id: string): JsModuleOptions {
    id = this.moduleOptionKey(id);
    const option = this.moduleOptionMap.get(id);
    if (!option) {
      const raw: JsModuleOptions = { meta: {} };
      this.moduleOptionMap.set(id, raw);
      return raw;
    }
    return option;
  }

  getModuleInfo(
    id: string,
    context: BindingPluginContext,
    moduleSideEffectsAccess: ModuleSideEffectsAccess,
    warnWriteAfterBuildEnd: (moduleId: string) => void,
  ): ModuleInfo | null {
    const bindingInfo = context.getModuleInfo(id);
    if (bindingInfo) {
      // Each call mints a fresh module-info box retaining the module's full
      // source, and a threadless WASI host may never run GC finalizers, so
      // hand out a plain-data snapshot and release the box immediately.
      return shouldEagerlyFreeOutputs()
        ? snapshotModuleInfo(
            bindingInfo,
            this.getModuleOption(id),
            this.getModuleSideEffectsAccess(context, moduleSideEffectsAccess),
            this.moduleSideEffectsOption,
            warnWriteAfterBuildEnd,
          )
        : transformModuleInfo(
            bindingInfo,
            this.getModuleOption(id),
            moduleSideEffectsAccess,
            this.moduleSideEffectsOption,
            warnWriteAfterBuildEnd,
          );
    }
    return null;
  }

  /**
   * Returns a build-scoped `ModuleSideEffectsAccess` on threadless WASI,
   * pinning at most one context box until `releaseRetainedOptionBoxes()`.
   * Falls back to `fallback` on lazy flavors or after terminal release
   * (e.g. `closeBundle`).
   */
  getModuleSideEffectsAccess(
    box: ModuleSideEffectsAccess & DroppableBox,
    fallback: ModuleSideEffectsAccess,
  ): ModuleSideEffectsAccess {
    if (!shouldEagerlyFreeOutputs() || this.#released) {
      return fallback;
    }
    if (this.#sideEffectsBox === undefined) {
      this.#sideEffectsBox = box;
      this.retainContextBox(box, {
        releaseBinding: () => {
          this.#sideEffectsBox = undefined;
        },
      });
    }
    return this.#sideEffectsAccess;
  }

  /**
   * Releases a per-hook context box unless it was promoted into
   * `#retainedContextBoxes` (e.g. as `#sideEffectsBox`), in which case
   * `releaseRetainedOptionBoxes()` releases it when the build settles.
   */
  releaseHookContextBox(box: DroppableBox): void {
    if (!this.#retainedContextBoxes.has(box)) {
      releaseOrDefer(box);
    }
  }

  getModuleIds(context: BindingPluginContext): ArrayIterator<string> {
    const moduleIds = context.getModuleIds();
    return moduleIds.values();
  }

  /**
   * Saves the options of one `this.resolve` call and returns its receipt. The receipt goes to Rust
   * as `BindingPluginContextResolveOptions.custom` (`Option<u32>`), and `bindingifyResolveId`
   * gets it back and looks the options up when a `resolveId` hook runs. Calls can overlap, so a
   * receipt must stay unique while its call is in flight. The map size cannot be the receipt: it
   * shrinks when an earlier call finishes, and the next call would get a live receipt.
   */
  saveResolveOptions(options: PluginContextResolveOptions): number {
    const receipt = freeReceipt(this.nextResolveOptionsReceipt, this.resolveOptionsMap);
    this.nextResolveOptionsReceipt = (receipt + 1) >>> 0;
    this.resolveOptionsMap.set(receipt, options);
    return receipt;
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

  /**
   * `null` clears the options of all modules (a full scan). Otherwise the given modules go back to
   * the options they started with, because their importers do not resolve them again.
   *
   * A partial scan also drops the resolutions saved before the last partial scan started. The
   * module that a resolution creates takes its options in the same scan, so these resolutions
   * created no module: `this.resolve` probes of Rust plugins, and imports of a module that
   * already exists. Without this, an HMR-only session would keep them until the next build ends.
   * The ones saved since then stay, so a module that this scan creates still finds its options.
   */
  resetModuleOptions(ids: string[] | null): void {
    if (!ids) {
      this.moduleOptionMap.clear();
      this.resolvedModuleOptionMap.clear();
      this.resolutionOptionsMap.clear();
      this.olderResolutionOptionsMap.clear();
      return;
    }
    this.olderResolutionOptionsMap = this.resolutionOptionsMap;
    this.resolutionOptionsMap = new Map();
    for (const id of ids) {
      const key = this.moduleOptionKey(id);
      const resolved = this.resolvedModuleOptionMap.get(key);
      if (resolved) {
        this.moduleOptionMap.set(key, { meta: { ...resolved.meta } });
      } else {
        this.moduleOptionMap.delete(key);
      }
    }
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
    this.loadModuleWithDependenciesPromiseMap.clear();
    // Watch mode reuses this object across rebuilds; entries hold the previous build's chunk data.
    this.outputCodeCache.clear();
    // Options of resolutions that created no module. The receipt counter keeps going, so a
    // receipt of this build never finds options of a later one.
    this.resolutionOptionsMap.clear();
    this.olderResolutionOptionsMap.clear();
    // `resolveOptionsMap` and its receipt counter stay as they are: a `this.resolve` call of the
    // previous build may still be in flight, and its receipt must not be given out again.
    this.#releaseOptionBoxes();
  }

  // Terminal release once a build, `scan()` or watcher close settles. The only
  // drain for context boxes; also covers option boxes when the invalidate
  // callback never fired (failed builds, `scan()`). Idempotent; no-op off
  // threadless WASI. Both parts run to the end before the first error, if
  // any, is rethrown.
  releaseRetainedOptionBoxes(): void {
    this.#released = true;
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

  // The cached wrappers read every value in their constructor and keep no binding object
  // (see internal-docs/bundler-data-lifecycle/implementation.md, "Normalized options objects"),
  // so the boxes have no reader left.
  #releaseOptionBoxes(): void {
    if (!shouldEagerlyFreeOutputs() || this.#retainedOptionBoxes.size === 0) {
      return;
    }
    const failure = releaseEach(this.#retainedOptionBoxes, (box) => box.dropInner(), undefined);
    this.#retainedOptionBoxes.clear();
    if (failure) {
      throw failure.error;
    }
  }
}
