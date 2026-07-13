import assert from 'node:assert';

// The pre-lowering chunk graph is acyclic; applying the wrap plan adds chunk A's `init_t` import
// (A -> C) through the wrapped barrel's excluded `export * from f` hop — the excluded-statement
// metadata walks every static import of the non-included `f` and finds `import { unused } from
// '../c/t.js'`. That closes a chunk cycle with the baseline C -> A edge from the eager reader's CJS
// carrier. The fixpoint projector resolves that hop through `f`'s resolved exports instead, and
// `unused` is not re-exported, so it misses A -> C. Without unifying the projection with the actual
// transitive-metadata routing, the eager reader's record-position interop trigger runs mid-cycle and
// reads A's not-yet-assigned `var require_carrier` — `TypeError: require_carrier is not a function`.
await import('./dist/main.js');

assert.strictEqual(
  globalThis.__carried,
  'CARRIED',
  `the eager interop read must observe the assigned CJS wrapper; got ${JSON.stringify(globalThis.__carried)}`,
);
assert.deepStrictEqual(
  globalThis.__result,
  { wv: 'WV', tv: 'TV', carried: 'CARRIED' },
  `strict order must deliver initialized values; got ${JSON.stringify(globalThis.__result)}`,
);
