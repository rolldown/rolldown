import assert from 'node:assert';

// Regression pin for the B (partial forwarder discharge) x C (emergent cycle) composition: the
// forwarder discharges its live `pv` hop (closing the emergent A <-> B cycle the fixpoint wraps),
// its dead `unused` hop triggers nothing, and the eager interop reader is deferred past both chunk
// bodies. Must deliver initialized values in both strict modes with no startup crash.
await import('./dist/main.js');

assert.strictEqual(
  globalThis.__carried,
  'CARRIED',
  `the eager interop read must observe the assigned CJS wrapper; got ${JSON.stringify(globalThis.__carried)}`,
);
assert.deepStrictEqual(
  globalThis.__result,
  { pv: 'PV', bv: 'BV', marker: 'F', carried: 'CARRIED' },
  `strict order must deliver initialized values; got ${JSON.stringify(globalThis.__result)}`,
);
