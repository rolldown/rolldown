import assert from 'node:assert';

globalThis.order = [];
const m = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.order, ['w', 'a', 'd']);
assert.strictEqual(m.read(), 'd');
