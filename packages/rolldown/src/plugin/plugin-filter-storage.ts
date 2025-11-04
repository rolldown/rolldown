import type { HookFilterExtension } from './index';

export interface PendingFilterOverride {
  resolveId?: Pick<HookFilterExtension<'resolveId'>, 'filter'>['filter'];
  load?: Pick<HookFilterExtension<'load'>, 'filter'>['filter'];
  transform?: Pick<HookFilterExtension<'transform'>, 'filter'>['filter'];
}

/**
 * Shared storage for plugin filter overrides that can be set before the binding context exists
 * (e.g., in the options hook) and applied later when the binding context is created.
 */
export class PluginFilterStorage {
  private pending: PendingFilterOverride | null = null;
  private applied = false;

  setPendingFilters(filters: PendingFilterOverride): void {
    this.pending = filters;
    this.applied = false;
  }

  getPendingFilters(): PendingFilterOverride | null {
    return this.pending;
  }

  markAsApplied(): void {
    this.applied = true;
  }

  hasBeenApplied(): boolean {
    return this.applied;
  }

  clearPendingFilters(): void {
    this.pending = null;
    this.applied = false;
  }
}
