import type { PersistentCacheModuleInfoUse } from '../log/logs';

/**
 * A use of a module value that gives the module task of the hook call no saved result. Another
 * module task can set the value at any time, so the cache cannot tell what a saved result needs.
 * A `this.resolve` result counts only when its `moduleSideEffects` came from `moduleOptionMap`.
 */
export interface ModuleInfoUse {
  reason: PersistentCacheModuleInfoUse;
  /** The key in `moduleOptionMap` of the module whose value the hook call used. */
  moduleKey: string;
}

/**
 * What one hook call does through `this.getModuleInfo` and through the result of
 * `this.resolve`.
 *
 * Only a `resolveId`, `resolveDynamicImport`, `load` or `transform` call has one, and it is used
 * only when the cache records the call.
 *
 * The persistent cache saves only the `meta` and the `moduleSideEffects` that a hook returns.
 * Each `PersistentCacheModuleInfoUse` gives the module task of the hook call no saved result.
 *
 * See internal-docs/persistent-cache/implementation.md.
 */
export class ModuleInfoAccess {
  /** `false` after the hook call ended. Later uses still reach the module, but do not count. */
  private open = true;
  private firstUse: ModuleInfoUse | undefined;

  get isOpen(): boolean {
    return this.open;
  }

  /** The hook call ended. */
  close(): void {
    this.open = false;
  }

  /** The hook call read or wrote `.meta` of the module `moduleKey`. */
  useMeta(moduleKey: string): void {
    this.use('meta', moduleKey);
  }

  /** The hook call assigned `moduleSideEffects` of the module `moduleKey`. */
  assignModuleSideEffects(moduleKey: string): void {
    this.use('moduleSideEffectsAssigned', moduleKey);
  }

  /** The hook call read `moduleSideEffects` of the module `moduleKey`. */
  readModuleSideEffects(moduleKey: string): void {
    this.use('moduleSideEffectsRead', moduleKey);
  }

  private use(reason: PersistentCacheModuleInfoUse, moduleKey: string): void {
    if (this.open) {
      this.firstUse ??= { reason, moduleKey };
    }
  }

  /** The first use, if the hook call made one. Call it when the hook call ends. It also closes this object. */
  collect(): ModuleInfoUse | undefined {
    this.close();
    return this.firstUse;
  }
}
