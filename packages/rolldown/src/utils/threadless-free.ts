import type { BindingOutputs } from '../binding.cjs';
import { getRuntimeSupport } from '../runtime-support';

// Threadless WASI hosts (@rolldown/browser, the wasi-single dist) rarely, and
// workerd never, run the GC finalizers that release native payloads: Wasm
// memory adds no JS-heap pressure. There, wrappers copy fields to JavaScript
// eagerly and drop the native side at once. Other builds stay lazy.
let eagerlyFreeOutputs: boolean | undefined;

export function shouldEagerlyFreeOutputs(): boolean {
  if (eagerlyFreeOutputs === undefined) {
    eagerlyFreeOutputs = getRuntimeSupport().threadlessWasi;
  }
  return eagerlyFreeOutputs;
}

/**
 * Release the native payload behind every chunk and asset of a
 * `BindingOutputs`, once its JavaScript wrappers are done reading.
 *
 * This does not reach the boxes `getModules()` mints; `OutputChunkImpl`
 * snapshots those itself.
 */
export function dropBindingOutputs(outputs: BindingOutputs): void {
  for (const chunk of outputs.chunks) {
    chunk.dropInner();
  }
  for (const asset of outputs.assets) {
    asset.dropInner();
  }
}

// A fire-and-forget `this.load()` / `this.resolve()` holds a shared napi
// borrow on its plugin-context box until the native promise settles, so an
// exclusive `dropInner()` in a hook's `finally` would throw. Such boxes go
// through `releaseOrDefer`: the last `endNativeCall` performs a requested drop.
export interface DroppableBox {
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
