import assert from 'node:assert/strict';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepEqual(globalThis.events, ['tag:true', 'optional:true', 'optional:true', 'tag:true']);
