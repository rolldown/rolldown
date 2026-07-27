import {
  createInstance,
  getDeferredRuntimeStats,
  WORKERD_WASM_MEMORY,
  type DeferredInstanceOptions,
  type DeferredRolldownInstance,
  type DeferredRuntimeStats,
} from './rolldown-binding.wasip1-deferred.js';
import type {
  BindingOutputs,
  BindingResult,
  ExternalMemoryStatus,
} from './rolldown-binding.wasip1.cjs';

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

export {
  build,
  createWorkerdBundle,
  type WorkerdBuildOptions,
  type WorkerdBundle,
  type WorkerdBundleOptions,
} from './workerd-build';
// Hosts without `node:async_hooks` (no nodejs_als/nodejs_compat flag) can
// provide their own storage; AsyncContextUnavailableError points users here.
export {
  configureAsyncContext,
  type AsyncContextProvider,
  type AsyncContextStorage,
} from './utils/async-context';
export type { InputOptions } from './options/input-options';
export type { OutputOptions } from './options/output-options';
export type { RolldownOutput, OutputChunk, OutputAsset } from './types/rolldown-output';
export type { BundleError } from './utils/error';
export type { Plugin, RolldownPlugin } from './plugin';

/** Per-item `dropInner()` statuses reported by {@linkcode freeOutputs}. */
export interface WorkerdFreeOutputsReport {
  /** Release status of each output chunk, in `outputs.chunks` order. */
  chunks: ExternalMemoryStatus[];
  /** Release status of each output asset, in `outputs.assets` order. */
  assets: ExternalMemoryStatus[];
}

/**
 * Release the native memory behind a completed `BindingBundler#generate()` or
 * `#write()` result.
 *
 * Each build's output payload (chunk code, sourcemaps, and the per-module
 * rendered sources) lives on the Wasm side and is normally reclaimed by a
 * JavaScript GC finalizer. workerd gives the surrounding isolate no JS-heap
 * pressure for Wasm memory and does not reliably run finalizers, so on this
 * surface every build's payload stays resident and sequential rebuilds grow
 * linear memory without bound (about 0.9 MiB per 300-module rebuild).
 *
 * Call this once you have finished reading an output (or copied the fields you
 * need). After it returns, the output's getters throw for anything not already
 * copied to JavaScript. Passing a binding-errors result is a safe no-op.
 *
 * @param outputs The settled result of `generate()`/`write()` on the raw
 *   workerd binding surface.
 * @returns Per-item release statuses; `freed: false` entries carry a `reason`
 *   (already freed, or other live native references).
 */
export function freeOutputs(
  outputs: BindingResult<BindingOutputs> | null | undefined,
): WorkerdFreeOutputsReport {
  const report: WorkerdFreeOutputsReport = { chunks: [], assets: [] };
  if (outputs === null || typeof outputs !== 'object') {
    return report;
  }
  if ('isBindingErrors' in outputs && outputs.isBindingErrors === true) {
    return report;
  }
  const { chunks, assets } = outputs as BindingOutputs;
  if (Array.isArray(chunks)) {
    for (const chunk of chunks) {
      report.chunks.push(chunk.dropInner());
    }
  }
  if (Array.isArray(assets)) {
    for (const asset of assets) {
      report.assets.push(asset.dropInner());
    }
  }
  return report;
}
