import assert from 'node:assert';

globalThis.order = [];
const m = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.order, ['e', 'f', 'n', 'x']);
assert.strictEqual(m.access(), 'e');
