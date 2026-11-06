import assert from 'node:assert';

globalThis.events = [];
await import('./dist/b.js');
assert.deepStrictEqual(globalThis.events, ['S body', 'B body S']);
await import('./dist/e.js');
assert.deepStrictEqual(globalThis.events, ['S body', 'B body S', 'E body']);
