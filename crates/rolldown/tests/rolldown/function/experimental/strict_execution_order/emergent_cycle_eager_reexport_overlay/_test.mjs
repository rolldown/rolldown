import assert from 'node:assert';

// The pre-lowering chunk graph is acyclic; applying the wrap plan adds the eager forwarder's
// cross-chunk `init_definer` overlay import (A -> B), which closes a chunk cycle with the baseline
// B -> A edge from the eager reader's CJS carrier. Chunk B's body then evaluates before chunk A's.
// The forwarder carries no `init_forwarder` of its own, so the fixpoint projector — which only
// walks importers that own an ESM `init_*` — skips it and misses the A -> B overlay edge. Without
// projecting overlay edges the eager reader's record-position interop trigger runs mid-cycle and
// reads A's not-yet-assigned `var require_carrier` — `TypeError: require_carrier is not a function`
// (vue-vben-admin's `qe is not a function`). Projecting the overlay edge wraps chunk B's eligible
// modules, deferring the read until after both chunk bodies.
await import('./dist/main.js');

assert.strictEqual(
  globalThis.__carried,
  'CARRIED',
  `the eager interop read must observe the assigned CJS wrapper; got ${JSON.stringify(globalThis.__carried)}`,
);
assert.deepStrictEqual(
  globalThis.__result,
  { pv: 'PV', marker: 'F', carried: 'CARRIED' },
  `strict order must deliver initialized values; got ${JSON.stringify(globalThis.__result)}`,
);
