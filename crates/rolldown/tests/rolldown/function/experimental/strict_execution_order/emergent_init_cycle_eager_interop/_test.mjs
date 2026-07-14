import assert from 'node:assert';

// The pre-lowering chunk graph is acyclic; applying the wrap plan adds the barrel's cross-chunk
// `init_pure` import (S -> H), which closes a chunk cycle with the baseline H -> S edge from the
// eager module's CJS carrier. Chunk H's body then evaluates before chunk S's. Without the
// emergent-cycle fixpoint the eager module's record-position interop trigger runs mid-cycle and
// reads S's not-yet-assigned `var require_carrier_cjs` — `TypeError: require_carrier_cjs is not a
// function` (vue-vben-admin's `qe is not a function`). The fixpoint wraps chunk H's eligible
// modules, deferring the read until after both chunk bodies.
await import('./dist/main.js');

assert.strictEqual(
  globalThis.__carried,
  'CARRIED',
  `the eager interop read must observe the assigned CJS wrapper; got ${JSON.stringify(globalThis.__carried)}`,
);
assert.deepStrictEqual(
  globalThis.__result,
  { pv: 'PV', bmark: 'B', carried: 'CARRIED' },
  `strict order must deliver initialized values; got ${JSON.stringify(globalThis.__result)}`,
);
