import assert from 'node:assert';

globalThis.__events = [];

const entry = await import('./dist/main.js');

assert.strictEqual(entry.x, 'x');
assert.deepStrictEqual(globalThis.__events, ['a:start', 'bridge', 'a:end', 'b', 'main:x']);
