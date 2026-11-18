import assert from 'node:assert';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepStrictEqual(globalThis.events, ['C body', 'A local h!', 'B h!']);
