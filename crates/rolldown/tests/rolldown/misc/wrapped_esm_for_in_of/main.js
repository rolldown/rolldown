import assert from 'node:assert';

const m = await import('./m.js');
assert.deepStrictEqual([m.x, m.k, m.a, m.b, m.d, m.nested], [1, 'a', 2, 3, 5, 4]);
