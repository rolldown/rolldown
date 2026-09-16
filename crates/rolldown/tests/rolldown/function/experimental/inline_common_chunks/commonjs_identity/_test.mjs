import assert from 'node:assert/strict';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepEqual(globalThis.events, ['cjs:init', 'a:1', 'same:true', 'b:2']);
