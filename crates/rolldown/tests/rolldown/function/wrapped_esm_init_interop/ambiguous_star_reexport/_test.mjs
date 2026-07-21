import assert from 'node:assert';

globalThis.__log = [];
const ns = await import('./dist/main.js');

// Runtime check: ambiguous x stays absent and both owners evaluate once; the snapshot pins that neither owner init is added at main's star re-export.
assert.strictEqual('x' in ns, false, 'ambiguous names are omitted from the namespace');
assert.deepStrictEqual(
  globalThis.__log,
  ['A', 'B', 'BEFORE_REQUIRE', 'MAIN'],
  'both ambiguous owners evaluate once before the main body',
);
