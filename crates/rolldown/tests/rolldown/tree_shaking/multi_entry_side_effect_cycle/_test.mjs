import assert from 'node:assert/strict';

globalThis.effects = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.effects, ['effect', 'main']);

await import('./dist/a.js');
assert.deepEqual(globalThis.effects, ['effect', 'main']);
