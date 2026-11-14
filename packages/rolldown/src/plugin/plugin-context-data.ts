import type {
  ModuleOptions,
  NormalizedInputOptions,
  NormalizedOutputOptions,
  OutputOptions,
  RolldownPlugin,
} from '..';
import type {
  BindingDeferSyncScanData,
  BindingModuleInfo,
  BindingOutputChunk,
  BindingPluginContext,
  BindingSavedModuleOptions,
  BindingSavedSideEffects,
} from '../binding.cjs';
import { type BindingNormalizedOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN, type RolldownLog } from '../log/logging';
import {
  logPersistentCacheModuleInfo,
  logPersistentCacheUnsavableMeta,
  type PersistentCacheModuleInfoUse,
} from '../log/logs';
import { NormalizedInputOptionsImpl } from '../options/normalized-input-options';
import { NormalizedOutputOptionsImpl } from '../options/normalized-output-options';
import { unreachable } from '../utils/misc';
import type { ModuleInfo } from '../types/module-info';
import { transformModuleInfo } from '../utils/transform-module-info';
import { parseModuleId, stringifyModuleId } from '../utils/module-id';
import type { CustomPluginOptions, RenderedChunkMeta } from '.';
import { ModuleInfoAccess } from './module-info-access';
import type { PluginContextResolveOptions } from './plugin-context';
import { isEmptyPlainObject, toSavableMetaEntries } from './savable-meta';

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

/** A hook call that the persistent cache records. */
export interface RecordedHookCall {
  pluginName: string;
  hookName: string;
}

/**
 * The module whose task gets no saved result. A nested `this.resolve` call runs its hooks for
 * the task that called it, so its importer can be another module. Rolldown warns only for a hook
 * call that the cache records, and such a call always has a module task.
 */
function warnedModuleId(context: BindingPluginContext): string {
  return context.cacheModuleId() ?? unreachable('A recorded hook call has no module task');
}

export class PluginContextData {
  moduleOptionMap: Map<string, ModuleOptions> = new Map();
  /**
   * The cache is `null` in parallel plugin workers. In a worker, each plugin has its own
   * `PluginContextData`, so the cache never finds a match. See `parallel-plugin-worker.ts`.
   */
  outputCodeCache: Map<string, OutputCodeCacheEntry> | null;
  resolveOptionsMap: Map<number, PluginContextResolveOptions> = new Map();
  /** The next `this.resolve` receipt. See `saveResolveOptions`. */
  private nextResolveOptionsReceipt = 0;
  loadModulePromiseMap: Map<string, Promise<void>> = new Map();
  renderedChunkMeta: RenderedChunkMeta | null = null;
  normalizedInputOptions: NormalizedInputOptions | null = null;
  normalizedOutputOptions: NormalizedOutputOptions | null = null;
  /** `experimental.importAttributes` */
  importAttributes = false;
  /** `experimental.cache` is set. */
  persistentCache = false;
  /**
   * For each key of `moduleOptionMap`, the stamp of the newest value from the persistent cache
   * that is already written into the map. See `syncSavedModuleOptions`.
   */
  private appliedSavedStamps: Map<string, number> = new Map();
  /**
   * The persistent cache warnings that were already printed, by code, plugin and hook. Never
   * cleared: each warning is printed once for each hook of each plugin.
   */
  private warnedCacheLogs: Set<string> = new Set();
  /**
   * With the persistent cache, the keys of `moduleOptionMap` whose `moduleSideEffects` a `load`
   * or `transform` hook of the module itself returned in this build. A write by anything else
   * removes the key. See `passOnModuleSideEffects`.
   */
  private ownSideEffects: Set<string> = new Set();
  /**
   * With the persistent cache, the `this.resolve` results whose `moduleSideEffects` came from
   * `moduleOptionMap` and was not replaced. See `trackResolvedSideEffects`.
   */
  private resolvedSideEffectsFromMap: WeakSet<object> = new WeakSet();
  /**
   * With the persistent cache, the keys in `moduleOptionMap` of the modules that have each `meta`
   * object. More than one module has the object when `updateModuleOption` gives a module the
   * object of another module. See `recordModuleOptions`.
   */
  private metaHolders: WeakMap<CustomPluginOptions, string[]> = new WeakMap();

  constructor(
    private onLog: LogHandler,
    private outputOptions: OutputOptions,
    private normalizedInputPlugins: RolldownPlugin[],
    private normalizedOutputPlugins: RolldownPlugin[],
    cacheOutputCode = true,
  ) {
    this.outputCodeCache = cacheOutputCode ? new Map() : null;
  }

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

  /** `ownHook`: the value comes from a `load` or `transform` hook of the module `id`. */
  updateModuleOption(id: string, option: ModuleOptions, ownHook = false): ModuleOptions {
    id = this.moduleOptionKey(id);
    if (this.persistentCache && option.moduleSideEffects != null) {
      if (ownHook) {
        this.ownSideEffects.add(id);
      } else {
        this.ownSideEffects.delete(id);
      }
    }
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
      this.addMetaHolder(option.meta, id);
      return option;
    }
    return existing;
  }

  private addMetaHolder(meta: CustomPluginOptions | null | undefined, key: string): void {
    // A `WeakMap` key must be an object, and a plugin can return any value as `meta`.
    if (
      this.persistentCache &&
      (typeof meta === 'object' || typeof meta === 'function') &&
      meta !== null
    ) {
      const holders = this.metaHolders.get(meta);
      if (holders) {
        holders.push(key);
      } else {
        this.metaHolders.set(meta, [key]);
      }
    }
  }

  /** A module other than `key` that has the object `meta`. */
  private otherMetaHolder(meta: CustomPluginOptions, key: string): string | undefined {
    return this.metaHolders.get(meta)?.find((holder) => holder !== key);
  }

  getModuleOption(id: string): ModuleOptions {
    id = this.moduleOptionKey(id);
    const option = this.moduleOptionMap.get(id);
    if (!option) {
      const raw: ModuleOptions = {
        moduleSideEffects: null,
        meta: {},
      };
      this.moduleOptionMap.set(id, raw);
      this.addMetaHolder(raw.meta, id);
      return raw;
    }
    return option;
  }

  /**
   * The persistent cache saves no result for the module task of the hook call that `context`
   * belongs to. Rust ignores the call when the cache does not record that hook call.
   */
  markNotCacheable(context: BindingPluginContext, reason: () => string): void {
    if (this.persistentCache) {
      context.markNotCacheable(reason());
    }
  }

  /**
   * Gives the `meta` and the `moduleSideEffects` that a hook returned for the module `id` to the
   * persistent cache. When the cache reuses the result of this hook call, the hook does not run,
   * and the cache gives the values back. See `syncSavedModuleOptions`.
   *
   * `meta` and `moduleSideEffects` are the values that the caller passes to `updateModuleOption`.
   * So the cache saves exactly what the hook call writes into `moduleOptionMap`.
   *
   * A `meta` that cannot be saved gives the module of the hook call no saved result. See
   * `toSavableMetaEntries`.
   *
   * Two modules can share one `meta` object. See `metaHolders`. A later change of the object then
   * shows up in both modules, and a saved result cannot hold that. So the module of the hook call
   * gets no saved result when the hook returns the `meta` object of another module, or when it
   * returns `meta` for a module that shares its object with another module.
   */
  recordModuleOptions(
    context: BindingPluginContext,
    hook: RecordedHookCall,
    id: string,
    meta: CustomPluginOptions | null | undefined,
    moduleSideEffects: ModuleOptions['moduleSideEffects'],
  ): void {
    if (!this.persistentCache) {
      return;
    }
    const key = this.moduleOptionKey(id);
    const current = this.moduleOptionMap.get(key)?.meta;
    const returnedHolder =
      meta != null && meta !== current ? this.otherMetaHolder(meta, key) : undefined;
    if (returnedHolder !== undefined) {
      this.markModuleInfoUse(context, hook, 'meta', returnedHolder);
      return;
    }
    // A hook can return the `meta` object of the module itself, for example with the result
    // of `this.resolve`. `updateModuleOption` then copies the object into itself, so the hook
    // call sets nothing.
    const hasMeta = meta != null && !isEmptyPlainObject(meta) && meta !== current;
    if (hasMeta && current !== undefined) {
      const sharedWith = this.otherMetaHolder(current, key);
      if (sharedWith !== undefined) {
        this.markModuleInfoUse(context, hook, 'meta', sharedWith);
        return;
      }
    }
    if (!hasMeta && moduleSideEffects == null) {
      return;
    }
    // The check of `meta` costs time, so it runs only for a hook call that is recorded.
    if (!context.isCacheRecording()) {
      return;
    }
    const entries = hasMeta ? toSavableMetaEntries(meta) : [];
    if (entries === undefined) {
      context.markNotCacheable(
        `hook \`${hook.hookName}\` of plugin \`${hook.pluginName}\` returned a \`meta\` value that cannot be saved`,
      );
      this.warnOnce(
        hook,
        logPersistentCacheUnsavableMeta(hook.pluginName, hook.hookName, warnedModuleId(context)),
      );
      return;
    }
    context.recordModuleOptions(key, entries, moduleSideEffects ?? undefined);
  }

  /** Prints `log` as a warning, once for each code, plugin and hook. */
  private warnOnce(hook: RecordedHookCall, log: RolldownLog): void {
    const key = `${log.code}\0${hook.pluginName}\0${hook.hookName}`;
    if (!this.warnedCacheLogs.has(key)) {
      this.warnedCacheLogs.add(key);
      this.onLog(LOG_LEVEL_WARN, log);
    }
  }

  /**
   * Starts to note what a `resolveId`, `resolveDynamicImport`, `load` or `transform` call does
   * through `this.getModuleInfo` and `this.resolve`. `undefined` without the persistent cache.
   * The caller must call `recordModuleInfoAccess` when the hook call ends, or `close` on the
   * result when it fails.
   */
  startModuleInfoAccess(): ModuleInfoAccess | undefined {
    return this.persistentCache ? new ModuleInfoAccess() : undefined;
  }

  /**
   * Gives what a hook call did through `this.getModuleInfo` and `this.resolve` to the
   * persistent cache. Call it when the hook call ends.
   */
  recordModuleInfoAccess(
    context: BindingPluginContext,
    hook: RecordedHookCall,
    access: ModuleInfoAccess,
  ): void {
    const use = access.collect();
    if (use) {
      this.markModuleInfoUse(context, hook, use.reason, use.moduleKey);
    }
  }

  /**
   * The hook call of `context` used a `meta` or `moduleSideEffects` value of the module `target`
   * that another hook can set. The module task gets no saved result, because that hook can set
   * another value in a later build. The plugin can write the hook in another way, so Rolldown
   * warns.
   */
  markModuleInfoUse(
    context: BindingPluginContext,
    hook: RecordedHookCall,
    use: PersistentCacheModuleInfoUse,
    target: string,
  ): void {
    if (!this.persistentCache || !context.isCacheRecording()) {
      return;
    }
    const what = {
      meta: `read, wrote or returned the \`meta\` of \`${target}\``,
      moduleSideEffectsRead: `read the \`moduleSideEffects\` of \`${target}\` through \`this.getModuleInfo\` or \`this.resolve\``,
      moduleSideEffectsAssigned: `assigned the \`moduleSideEffects\` of \`${target}\` through \`this.getModuleInfo\``,
    }[use];
    context.markNotCacheable(`hook \`${hook.hookName}\` of plugin \`${hook.pluginName}\` ${what}`);
    this.warnOnce(
      hook,
      logPersistentCacheModuleInfo(
        hook.pluginName,
        hook.hookName,
        warnedModuleId(context),
        target,
        use,
      ),
    );
  }

  /**
   * The hook call of `context` gave Rust a `moduleSideEffects` value of the module `target` that
   * another module task set: it returned a `this.resolve` result with that value (`returned`), or
   * a result without `moduleSideEffects`, and the wrapper passed on the value (`passedOn`). The
   * module task gets no saved result, as in `markModuleInfoUse`. Rolldown does not warn: this is
   * the usual way to write a hook, and the plugin cannot know which value to return instead.
   */
  markSideEffectsPassedOn(
    context: BindingPluginContext,
    hook: RecordedHookCall,
    how: 'returned' | 'passedOn',
    target: string,
  ): void {
    if (!this.persistentCache || !context.isCacheRecording()) {
      return;
    }
    const what = {
      returned: `returned a \`this.resolve\` result with the \`moduleSideEffects\` that another hook set for \`${target}\``,
      passedOn: `passed on the \`moduleSideEffects\` that another hook set for \`${target}\``,
    }[how];
    context.markNotCacheable(`hook \`${hook.hookName}\` of plugin \`${hook.pluginName}\` ${what}`);
  }

  /**
   * Writes the values that the persistent cache holds for the module `id` into
   * `moduleOptionMap`. They come from saved results that the cache reused: the hooks that set
   * them did not run. Rust does not call JavaScript for each reused module. The JavaScript side
   * takes the values when it reads or changes the options of a module.
   *
   * Call it before a hook result is written into the options of `id`, and before they are read.
   * A saved value is older than the result of a hook that runs now, so it must be written first.
   */
  syncSavedModuleOptions(
    id: string,
    context: BindingPluginContext,
    /** The stamp of the newest value that the cache holds for the module, if Rust gave it. */
    newestStamp?: number,
  ): void {
    if (this.persistentCache) {
      const key = this.moduleOptionKey(id);
      const applied = this.appliedSavedStamps.get(key) ?? 0;
      if (newestStamp === undefined || newestStamp > applied) {
        this.applySavedModuleOptions(key, context.getSavedModuleOptions(key, applied));
      }
    }
  }

  /** Like `syncSavedModuleOptions`, with the module info that Rust gave for the module `id`. */
  syncSavedModuleOptionsFromInfo(id: string, info: BindingModuleInfo): void {
    if (this.persistentCache) {
      const key = this.moduleOptionKey(id);
      this.applySavedModuleOptions(
        key,
        info.savedModuleOptions(this.appliedSavedStamps.get(key) ?? 0),
      );
    }
  }

  /**
   * A saved key of `meta` replaces the same key, and the other keys stay, as when a hook returns
   * `meta`. See `updateModuleOption`. A saved `moduleSideEffects` is a returned value, so it sets
   * `invalidate` to `false`, as `updateModuleOption` does for a returned value.
   */
  private applySavedModuleOptions(key: string, saved: BindingSavedModuleOptions | null): void {
    if (!saved) {
      return;
    }
    this.appliedSavedStamps.set(key, saved.stamp);
    let option = this.moduleOptionMap.get(key);
    if (!option) {
      option = { moduleSideEffects: null, meta: {} };
      this.moduleOptionMap.set(key, option);
      this.addMetaHolder(option.meta, key);
    }
    for (const entry of saved.meta) {
      // An entry without JSON text stands for the value `undefined`.
      option.meta[entry.key] = entry.json == null ? undefined : JSON.parse(entry.json);
    }
    if (saved.moduleSideEffects != null) {
      option.moduleSideEffects = saved.moduleSideEffects;
      option.invalidate = false;
      this.ownSideEffects.delete(key);
    }
  }

  /**
   * The modules whose `moduleSideEffects` was assigned on a module info, for Rolldown's
   * `deferSyncScanData` callback. No hook result holds such a value.
   *
   * `savedSideEffects` are the `moduleSideEffects` values that hooks returned, from saved
   * results that the persistent cache reused. `moduleOptionMap` has such a value only after
   * something read the module. A value with a newer stamp than the applied one is not in the map
   * yet. It is also newer than the value in the map, because every write to the map takes the
   * saved values first. So it replaced an older assignment in the map, and Rolldown already has
   * it from the saved result.
   */
  deferSyncScanData(savedSideEffects: BindingSavedSideEffects[]): BindingDeferSyncScanData[] {
    let notApplied: Set<string> | undefined;
    for (const saved of savedSideEffects) {
      if (saved.stamp > (this.appliedSavedStamps.get(saved.id) ?? 0)) {
        (notApplied ??= new Set()).add(saved.id);
      }
    }
    const ret: BindingDeferSyncScanData[] = [];
    this.moduleOptionMap.forEach((value, key) => {
      if (value.invalidate && !notApplied?.has(key)) {
        ret.push({ id: key, sideEffects: value.moduleSideEffects ?? undefined });
      }
    });
    return ret;
  }

  /**
   * A `load` or `transform` hook of the module `id` returned no `moduleSideEffects`, and its
   * wrapper passes on the value in `moduleOptionMap` to Rust. The value is not a result of the
   * hook, so the persistent cache must not save it as one:
   * - An earlier `load` or `transform` hook of the module returned it. The saved result already
   *   has it.
   * - Otherwise another module task set it. That task can set another value in a later build,
   *   so the module task gets no saved result.
   */
  passOnModuleSideEffects(context: BindingPluginContext, hook: RecordedHookCall, id: string): void {
    if (!this.persistentCache) {
      return;
    }
    const key = this.moduleOptionKey(id);
    if (this.ownSideEffects.has(key)) {
      context.ignoreNextHookSideEffects();
    } else {
      this.markSideEffectsPassedOn(context, hook, 'passedOn', key);
    }
  }

  /**
   * `access` is set in a `resolveId`, `resolveDynamicImport`, `load` or `transform` call. When
   * the persistent cache records the call, `access` notes what the hook does with the result.
   * See `ModuleInfoAccess`.
   */
  getModuleInfo(
    id: string,
    context: BindingPluginContext,
    access?: ModuleInfoAccess,
  ): ModuleInfo | null {
    const bindingInfo = context.getModuleInfo(id);
    if (bindingInfo) {
      this.syncSavedModuleOptionsFromInfo(id, bindingInfo);
      const option = this.getModuleOption(id);
      const info = transformModuleInfo(bindingInfo, option);
      return this.proxyModuleInfo(id, info, context, this.openAccess(context, access));
    }
    return null;
  }

  /** `access`, when the hook call has not ended and the persistent cache records it. */
  private openAccess(
    context: BindingPluginContext,
    access: ModuleInfoAccess | undefined,
  ): ModuleInfoAccess | undefined {
    return access?.isOpen && context.isCacheRecording() ? access : undefined;
  }

  /**
   * `target` is a module info or a `this.resolve` result that a hook call gets, and `target.meta`
   * is the `meta` object of the module `id`.
   *
   * With `access`, `target.meta` becomes a getter and a setter that tell `access` about each use
   * until the hook call ends. Most hook calls never use `meta`, and they pay nothing more.
   *
   * Returns what `target.meta` holds now, without a use of the property.
   */
  private trackMeta(
    target: { meta: CustomPluginOptions },
    id: string,
    access: ModuleInfoAccess | undefined,
  ): () => CustomPluginOptions {
    if (!access) {
      return () => target.meta;
    }
    const key = this.moduleOptionKey(id);
    let given = target.meta;
    // The property stays enumerable. So a spread or `JSON.stringify` of `target` is a use too.
    Object.defineProperty(target, 'meta', {
      get: () => {
        access.useMeta(key);
        return given;
      },
      set: (value: CustomPluginOptions) => {
        access.useMeta(key);
        given = value;
      },
    });
    return () => given;
  }

  /** Like `trackMeta`, for the result of `this.resolve` in the hook call of `access`. */
  trackResolvedMeta(
    resolved: { meta: CustomPluginOptions },
    id: string,
    context: BindingPluginContext,
    access: ModuleInfoAccess | undefined,
  ): void {
    this.trackMeta(resolved, id, this.openAccess(context, access));
  }

  /**
   * `resolved` is the result of `this.resolve` in the hook call of `access`. The module task gets
   * no saved result when the hook call reads `resolved.moduleSideEffects` before it assigns a
   * value. Another module task can set the value in `moduleOptionMap` in a later build, also when
   * the map has no value now.
   *
   * `fromMap`: the value came from `moduleOptionMap`. Then the module task also gets no saved
   * result when the hook returns `resolved` with it. Rolldown reads a returned value after the
   * hook call ended, so the wrapper checks that with `isResolvedSideEffectsFromMap`.
   *
   * There is no exception for a value that is the same as the one from the resolver. The other
   * module task can set another value in a later build.
   */
  trackResolvedSideEffects(
    resolved: { moduleSideEffects: ModuleOptions['moduleSideEffects'] },
    id: string,
    context: BindingPluginContext,
    access: ModuleInfoAccess | undefined,
    fromMap: boolean,
  ): void {
    if (!this.persistentCache) {
      return;
    }
    if (fromMap) {
      this.resolvedSideEffectsFromMap.add(resolved);
    }
    const open = this.openAccess(context, access);
    if (!open) {
      return;
    }
    const key = this.moduleOptionKey(id);
    let value = resolved.moduleSideEffects;
    let assigned = false;
    // The property stays enumerable. So a spread of `resolved` is a read too.
    Object.defineProperty(resolved, 'moduleSideEffects', {
      get: () => {
        if (!assigned) {
          open.readModuleSideEffects(key);
        }
        return value;
      },
      set: (v: ModuleOptions['moduleSideEffects']) => {
        assigned = true;
        this.resolvedSideEffectsFromMap.delete(resolved);
        value = v;
      },
    });
  }

  /** `ret` is a `this.resolve` result that still has a `moduleSideEffects` from `moduleOptionMap`. */
  isResolvedSideEffectsFromMap(ret: object): boolean {
    return this.persistentCache && this.resolvedSideEffectsFromMap.has(ret);
  }

  proxyModuleInfo(
    id: string,
    info: ModuleInfo,
    context: BindingPluginContext,
    access?: ModuleInfoAccess,
  ): ModuleInfo {
    let moduleSideEffects = info.moduleSideEffects;
    const givenMeta = this.trackMeta(info, id, access);
    // A read counts for the module of the hook call too: another module task can set its value.
    // The property stays enumerable. So a spread of `info` is a read too.
    Object.defineProperty(info, 'moduleSideEffects', {
      get: () => {
        access?.readModuleSideEffects(this.moduleOptionKey(id));
        return moduleSideEffects;
      },
      set: (v: any) => {
        // A plugin can keep `info` and assign later. A saved value that came in between is
        // older than this assignment, so it must be written first.
        this.syncSavedModuleOptions(id, context);
        access?.assignModuleSideEffects(this.moduleOptionKey(id));
        this.updateModuleOption(id, {
          moduleSideEffects: v,
          meta: givenMeta(),
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

  /**
   * Saves the options of one `this.resolve` call and returns its receipt. The receipt goes to Rust
   * as `BindingPluginContextResolveOptions.custom` (`Option<u32>`), and `bindingifyResolveId`
   * gets it back and looks the options up when a `resolveId` hook runs. Calls can overlap, so a
   * receipt must stay unique while its call is in flight. The map size cannot be the receipt: it
   * shrinks when an earlier call finishes, and the next call would get a live receipt.
   */
  saveResolveOptions(options: PluginContextResolveOptions): number {
    let receipt = this.nextResolveOptionsReceipt;
    // The counter wraps at 2^32 so the receipt stays a valid `u32`. After a wrap, skip receipts
    // that are still in flight.
    while (this.resolveOptionsMap.has(receipt)) {
      receipt = (receipt + 1) >>> 0;
    }
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

  clear(): void {
    this.renderedChunkMeta = null;
    this.loadModulePromiseMap.clear();
    // Watch mode reuses this object across rebuilds; entries hold the previous build's chunk data.
    this.outputCodeCache?.clear();
    // A value from a hook of the module in this build is from another module task in the next.
    this.ownSideEffects.clear();
    // `resolveOptionsMap` and its receipt counter stay as they are: a `this.resolve` call of the
    // previous build may still be in flight, and its receipt must not be given out again.
  }
}
