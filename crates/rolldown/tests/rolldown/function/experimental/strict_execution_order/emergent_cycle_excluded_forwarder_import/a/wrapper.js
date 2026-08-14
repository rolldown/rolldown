// Order-wrapped barrel in chunk A. Its own pure-call export `wv` makes it order-sensitive so the
// first root's deviation wraps it. Beside `wv` it carries a tree-shaken `export * from '../f.js'`:
// nothing consumes `f`'s exports, so the statement is excluded — but because this barrel is
// order-wrapped, the excluded-statement metadata still forwards through the hop, walking the
// non-included `f`'s static imports and forwarding `init_wrapper` to the `init_t` it finds there.
// That is the cross-chunk A -> C edge the projector's resolved-exports walk cannot see.
export * from '../f.js';

function mkWv() {
  return 'WV';
}

// Pure call initializer: order-sensitive (so the deviation can flag this module) yet side-effect
// free and not const-inlinable (so the binding stays materialized).
export const wv = /* @__PURE__ */ mkWv();
