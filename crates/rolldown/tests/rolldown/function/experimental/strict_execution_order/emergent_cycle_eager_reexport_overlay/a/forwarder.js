// Eager pure re-export barrel in chunk A. It has no side effect or order-sensitive initializer of
// its own, so on-demand wrapping does not plan it. The entry's live `pv` obligation must resolve
// directly to `definer`; the retained-path overlay must not additionally reference
// `init_definer`, because that would manufacture a cross-chunk A -> B edge. Wrap-all may retain a
// conservative `init_forwarder` path and handles the resulting cycle through wrapper projection.
//
// The exported function declaration is hoisted and contributes nothing to an `__esm` closure, so
// it keeps the barrel a real, retained module in chunk A (its named re-export is not inlined away)
// without making it order-sensitive — the barrel stays eager. Consuming `marker` in the entry is
// what pins the barrel; the retained `export { pv } from` beside it exercises obligation routing.
export function marker() {
  return 'F';
}

export { pv } from '../b/definer.js';
