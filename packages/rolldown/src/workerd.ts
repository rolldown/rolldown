import {
  createInstance,
  getDeferredRuntimeStats,
  WORKERD_WASM_MEMORY,
  type DeferredInstanceOptions,
  type DeferredRolldownInstance,
  type DeferredRuntimeStats,
} from './rolldown-binding.wasip1-deferred.js';

// See internal-docs/async-runtime/implementation.md.
export type WorkerdInstanceOptions = DeferredInstanceOptions;
export type WorkerdRolldownInstance = DeferredRolldownInstance;
export type WorkerdRuntimeStats = DeferredRuntimeStats;

export { createInstance, WORKERD_WASM_MEMORY };

/** Compatibility alias for the managed factory. */
export const instantiate: typeof createInstance = createInstance;

/**
 * Report loader-local managed-instance counts and the declared initial Wasm
 * address space. Use platform memory telemetry for committed memory and quota
 * enforcement.
 */
export const getWorkerdRuntimeStats: typeof getDeferredRuntimeStats = getDeferredRuntimeStats;
