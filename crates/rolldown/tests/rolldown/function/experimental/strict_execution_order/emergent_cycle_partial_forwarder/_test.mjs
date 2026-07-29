import assert from 'node:assert';

// Regression pin for partial-forwarder routing: the live `pv` hop reaches `init_definer`, while
// the dead `unused` hop triggers nothing. On-demand mode must not add a phantom forwarder edge;
// wrap-all may conservatively defer the eager interop reader. Both modes must deliver initialized
// values with no startup crash.
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
