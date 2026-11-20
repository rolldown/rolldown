import assert from 'node:assert';

globalThis.order = [];
const m = await import('./dist/main.js');

assert.deepStrictEqual(globalThis.order, ['y', 'b', 'z']);
assert.strictEqual(m.access(), 'foo');
assert.deepStrictEqual(globalThis.order, ['y', 'b', 'z', 'a sees function']);
