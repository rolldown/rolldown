import assert from 'node:assert';

globalThis.order = [];
const m = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.order, ['b', 'n', 'a', 'x']);
assert.strictEqual(m.access(), 'a');
