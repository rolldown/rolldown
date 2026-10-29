import assert from 'node:assert';

globalThis.logs = [];
await import('./dist/e1.js');
assert.deepStrictEqual(globalThis.logs, ['m0', 'm2', 'm3', 'm1']);
delete globalThis.logs;
