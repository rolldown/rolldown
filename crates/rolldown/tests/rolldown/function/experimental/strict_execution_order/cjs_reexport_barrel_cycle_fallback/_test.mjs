import assert from 'node:assert';

globalThis.__events = [];

const entry = await import('./dist/main.js');

assert.strictEqual(entry.value, 'value');
assert.deepStrictEqual(globalThis.__events, ['cjs', 'leaf', 'main:value']);
