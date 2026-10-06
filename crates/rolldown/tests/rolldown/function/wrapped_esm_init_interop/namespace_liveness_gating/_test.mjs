import assert from 'node:assert';

globalThis.__log = [];
await import('./dist/main.js');

// Runtime smoke check: the namespace value remains usable and each leaf evaluates once; the snapshot pins exact init routing.
assert.deepStrictEqual(
  globalThis.__log,
  ['A', 'B', 'BEFORE_REQUIRE:a', 'MAIN'],
  'the namespace value remains usable and each leaf evaluates once',
);
