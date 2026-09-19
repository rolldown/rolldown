import assert from 'node:assert/strict';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepEqual(globalThis.events, ['count:1', 'same:true', 'live:1', 'count:2']);
