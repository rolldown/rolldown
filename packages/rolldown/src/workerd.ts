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
import { snapshotChunkModules } from './utils/transform-rendered-chunk';

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
 * `#write()` result: the chunk and asset boxes it holds.
 *
 * The payload lives on the Wasm side and is normally reclaimed by a GC
 * finalizer, which workerd does not run reliably: without this call every
 * build's payload stays resident and sequential rebuilds grow linear memory
 * without bound (about 0.9 MiB per 300-module rebuild).
 *
 * Call this once you have finished reading an output (or copied the fields you
 * need). After it returns, the output's getters throw for anything not already
 * copied to JavaScript. Passing a binding-errors result is a safe no-op.
 *
 * Not covered: `chunk.getModules()`. Every call mints fresh
 * `BindingRenderedModule` boxes, each holding its own reference to that
 * module's rendered source (and sourcemap). This function cannot reach them,
 * workerd never finalizes them, and the chunk's own status stays `freed: true`
 * because they reference the modules, not the chunk. Read modules through
 * {@linkcode snapshotModules}, or call `dropInner()` on each box yourself.
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

/**
 * Copy the result of `chunk.getModules()` on the raw workerd binding surface to
 * plain JavaScript data and release every `BindingRenderedModule` box in it.
 *
 * Returns a `{ [moduleId]: { code, renderedLength, renderedExports } }` map
 * that stays readable after the boxes, the chunk, and the instance are gone.
 * The boxes in `modules.values` are freed on return: reading them afterwards
 * throws, and their `dropInner()` reports "already been freed".
 *
 * Raw-path recipe for one build:
 *
 * ```js
 * const result = await bundler.generate(options);
 * const modules = snapshotModules(result.chunks[0].getModules());
 * freeOutputs(result);
 * ```
 *
 * The high-level `build()` and `createWorkerdBundle()` do this for you.
 */
export const snapshotModules: typeof snapshotChunkModules = snapshotChunkModules;
