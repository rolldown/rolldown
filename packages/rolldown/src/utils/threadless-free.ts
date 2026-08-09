import type { BindingOutputs } from '../binding.cjs';
import { getRuntimeSupport } from '../runtime-support';

// Threadless WASI consumers (@rolldown/browser, the wasi-single dist) can run
// in engines that rarely — for workerd, never — run the GC finalizers emnapi
// relies on to release each build's native output payload: Wasm memory adds no
// JS-heap pressure, so V8 sees no reason to collect. Anything owned by a
// finalizer (output chunk/asset boxes, per-module rendered sources, hook-copy
// bundles, and every hook-input box: rendered chunks, module infos, plugin
// contexts, normalized options) then stays resident forever, growing linear
// memory by roughly one output payload per rebuild. For that flavor the
// wrappers copy all output fields to JavaScript eagerly and drop the native
// side immediately — hook inputs once their hook invocation settles. Native
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

// A fire-and-forget `this.load()` / `this.resolve()` — documented, un-awaited
// usage — keeps a napi SHARED borrow on its `BindingPluginContext` box until
// the native promise settles, and its JS continuation legally re-enters the
// box afterwards. A hook wrapper's `finally` calling `dropInner()` (an
// exclusive `&mut self` borrow) while that call is in flight makes the napi
// borrow checker throw INSIDE the finally, failing the build in the plugin's
// name. So plugin-context boxes are never dropped directly: wrappers request
// the release through `releaseOrDefer`, `PluginContextImpl.load/resolve`
// bracket each native call plus its continuation with `beginNativeCall` /
// `endNativeCall`, and when a release was requested while calls were in
// flight, the LAST call to finish performs the drop. Sync-only boxes cannot
// hold a pending borrow and keep calling `dropInner()` directly.
interface DroppableBox {
  dropInner(): unknown;
}

interface InFlightNativeCalls {
  pending: number;
  dropRequested: boolean;
}

const inFlightNativeCalls: WeakMap<DroppableBox, InFlightNativeCalls> = new WeakMap();

export function beginNativeCall(ctx: DroppableBox): void {
  if (!shouldEagerlyFreeOutputs()) {
    return;
  }
  const state = inFlightNativeCalls.get(ctx);
  if (state) {
    state.pending += 1;
  } else {
    inFlightNativeCalls.set(ctx, { pending: 1, dropRequested: false });
  }
}

export function endNativeCall(ctx: DroppableBox): void {
  if (!shouldEagerlyFreeOutputs()) {
    return;
  }
  const state = inFlightNativeCalls.get(ctx);
  if (!state || state.pending === 0) {
    return;
  }
  state.pending -= 1;
  if (state.pending === 0 && state.dropRequested) {
    inFlightNativeCalls.delete(ctx);
    ctx.dropInner();
  }
}

export function releaseOrDefer(ctx: DroppableBox): void {
  if (!shouldEagerlyFreeOutputs()) {
    return;
  }
  const state = inFlightNativeCalls.get(ctx);
  if (state && state.pending > 0) {
    state.dropRequested = true;
    return;
  }
  inFlightNativeCalls.delete(ctx);
  ctx.dropInner();
}
