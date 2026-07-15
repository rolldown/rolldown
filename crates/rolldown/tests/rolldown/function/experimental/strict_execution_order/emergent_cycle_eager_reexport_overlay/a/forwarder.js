// Eager pure re-export barrel in chunk A. It has no side effect and no order-sensitive
// initializer of its own, so on-demand wrapping never plans it — yet its retained
// `export { pv } from '../b/definer.js'` re-export of the order-wrapped `definer` makes the
// lowering give it an `OrderImportOverlay` that references `init_definer` and emit that
// `init_definer()` call in chunk A's body. That is a *new* cross-chunk import A -> B carried by
// the overlay, not by any `init_forwarder` the fixpoint projector inspects (it skips every
// importer without its own ESM `init_*`). Together with the baseline B -> A edge this closes the
// emergent chunk cycle the one-shot analysis never saw.
//
// The exported function declaration is hoisted and contributes nothing to an `__esm` closure, so
// it keeps the barrel a real, retained module in chunk A (its named re-export is not inlined away)
// without making it order-sensitive — the barrel stays eager. Consuming `marker` in the entry is
// what pins the barrel; the retained `export { pv } from` beside it is what creates the overlay.
export function marker() {
  return 'F';
}

export { pv } from '../b/definer.js';
