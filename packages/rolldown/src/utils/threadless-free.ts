import type { BindingOutputs } from '../binding.cjs';
import { getRuntimeSupport } from '../runtime-support';

// Threadless WASI consumers (@rolldown/browser, the wasi-single dist) can run
// in engines that rarely — for workerd, never — run the GC finalizers emnapi
// relies on to release native payloads: Wasm memory adds no JS-heap pressure,
// so V8 sees no reason to collect, and everything a finalizer owns stays
// resident forever. On that flavor the wrappers copy all fields to JavaScript
// eagerly and drop the native side at once (hook inputs once their invocation
// settles). Native and threaded-WASI builds stay lazy: finalization works
// there and the eager copy costs performance.
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
 * `BindingOutputs`, once its JavaScript wrappers are done reading.
 *
 * `src/workerd.ts` `freeOutputs()` keeps its own equivalent loop on purpose: it
 * reports per-item statuses and must work against a raw binding result.
 */
export function dropBindingOutputs(outputs: BindingOutputs): void {
  for (const chunk of outputs.chunks) {
    chunk.dropInner();
  }
  for (const asset of outputs.assets) {
    asset.dropInner();
  }
}

// A fire-and-forget `this.load()` / `this.resolve()` holds a napi SHARED
// borrow on its `BindingPluginContext` box until the native promise settles,
// so a hook wrapper's `finally` calling `dropInner()` (exclusive `&mut self`)
// would make the napi borrow checker throw inside that finally, failing the
// build in the plugin's name. Plugin-context boxes therefore go through
// `releaseOrDefer`: `PluginContextImpl.load/resolve` bracket each native call
// and its continuation with `beginNativeCall`/`endNativeCall`, and the LAST
// call to finish performs a requested drop. Sync-only boxes cannot hold a
// pending borrow and keep calling `dropInner()` directly.
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
