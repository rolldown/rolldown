import assert from 'node:assert';

const e = 'main';
const f = 'main';
const g = 'main';
const m = await import('./m.js');
assert.deepStrictEqual(
  [e, f, g, m.e, m.f, m.g, globalThis.caught],
  ['main', 'main', 'main', 10, undefined, undefined, 2],
);
