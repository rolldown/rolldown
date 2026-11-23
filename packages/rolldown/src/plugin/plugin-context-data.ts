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
import {
  type ModuleSideEffectsOptionFn,
  transformModuleInfo,
} from '../utils/transform-module-info';
import { parseModuleId, stringifyModuleId } from '../utils/module-id';
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
  normalizedInputOptions: NormalizedInputOptions | null = null;
  normalizedOutputOptions: NormalizedOutputOptions | null = null;
  /** `experimental.importAttributes` */
  importAttributes = false;
  /**
   * `treeshake.moduleSideEffects` when it is a function. The `moduleSideEffects` getter and setter
   * pass it to Rust, which calls it only when the value needs it.
   */
  moduleSideEffectsOption: ModuleSideEffectsOptionFn | undefined = undefined;

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
    warnWriteAfterBuildEnd: (moduleId: string) => void,
  ): ModuleInfo | null {
    const bindingInfo = context.getModuleInfo(id);
    if (bindingInfo) {
      return transformModuleInfo(
        bindingInfo,
        this.getModuleOption(id),
        context,
        this.moduleSideEffectsOption,
        warnWriteAfterBuildEnd,
      );
    }
    return null;
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
  }
}
