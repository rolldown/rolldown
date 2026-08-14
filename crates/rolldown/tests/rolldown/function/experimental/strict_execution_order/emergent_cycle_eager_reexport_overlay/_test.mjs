import assert from 'node:assert';

// On-demand routing gives the entry direct ownership of `init_definer` and does not leave a
// duplicate direct-wrapper reference on the forwarder's non-empty retained path. That avoids a
// phantom A -> B edge and the artificial cycle it would close with B's CJS import from A.
// Wrap-all keeps the conservative wrapper route and safely defers the carrier read. Both modes
// must observe initialized values.
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
