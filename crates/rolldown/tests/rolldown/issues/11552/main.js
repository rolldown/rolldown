import assert from 'node:assert';

const m = await import('./m.js');

assert.strictEqual(m.a, 1);
assert.strictEqual(m.b, 2);
assert.strictEqual(m.c, 3);
