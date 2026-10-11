import assert from 'node:assert/strict';

globalThis.marker = 'before';
await assert.rejects(import('./dist/main.js'), { message: 'before' });
