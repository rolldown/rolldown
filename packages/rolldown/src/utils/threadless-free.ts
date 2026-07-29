import type { BindingOutputs } from '../binding.cjs';
import { getRuntimeSupport } from '../runtime-support';

// Threadless WASI consumers (@rolldown/browser, the wasi-single dist) can run
// in engines that rarely — for workerd, never — run the GC finalizers emnapi
// relies on to release each build's native output payload: Wasm memory adds no
// JS-heap pressure, so V8 sees no reason to collect. Anything owned by a
// finalizer (output chunk/asset boxes, per-module rendered sources, hook-copy
// bundles) then stays resident forever, growing linear memory by roughly one
// output payload per rebuild. For that flavor the wrappers copy all output
// fields to JavaScript eagerly and drop the native side immediately. Native
// (and threaded-WASI) builds keep the lazy fields: finalization works there
// and the eager copy costs performance.
let eagerlyFreeOutputs: boolean | undefined;

export function shouldEagerlyFreeOutputs(): boolean {
  if (eagerlyFreeOutputs === undefined) {
    try {
      eagerlyFreeOutputs = getRuntimeSupport().threadlessWasi;
    } catch {
      // A binding without a readable capability report keeps the historical
      // lazy behavior.
      eagerlyFreeOutputs = false;
    }
  }
  return eagerlyFreeOutputs;
}

/**
 * Release the native payload behind every chunk and asset of a
 * `BindingOutputs`. Used on the threadless-WASI flavor for output copies whose
 * JavaScript wrappers are done reading (the eagerly-materialized final result,
 * or a generateBundle/writeBundle hook's marshaled bundle copy).
 *
 * The workerd entry (`src/workerd.ts` `freeOutputs()`) intentionally keeps its
 * own equivalent loop: it reports per-item statuses and must stay usable
 * against a raw binding result.
 */
export function dropBindingOutputs(outputs: BindingOutputs): void {
  for (const chunk of outputs.chunks) {
    chunk.dropInner();
  }
  for (const asset of outputs.assets) {
    asset.dropInner();
  }
}
