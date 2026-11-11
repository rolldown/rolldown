import assert from 'node:assert';

const m = await import('./m.js');
assert.deepStrictEqual([m.a, m.b, m.c, m.k], [1, 2, 3, 'z']);
