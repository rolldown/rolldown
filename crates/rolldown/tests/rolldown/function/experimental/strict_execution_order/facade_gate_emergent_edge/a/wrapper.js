// Order-wrapped barrel in chunk A. `wv` (a pure call) keeps it order-sensitive so the root's
// deviation wraps it; the pure body means the excluded `export * from '../f.js'` below is
// tree-shaken and `f` dropped. Because the barrel is order-wrapped, the excluded-statement metadata
// still forwards `init_t` through `f` — the projected, baseline-invisible chunk edge into `sec`'s
// chunk.
export * from '../f.js';
function mkWv() {
  return 'WV';
}
export const wv = /* @__PURE__ */ mkWv();
