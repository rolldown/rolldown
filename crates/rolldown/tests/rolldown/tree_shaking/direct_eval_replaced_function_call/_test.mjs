import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, ['local', 'let', 'var', 'function', 'empty function', 'let']);
