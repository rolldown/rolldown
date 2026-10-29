import assert from 'node:assert/strict';

await assert.rejects(import('./dist/main-var.js'), TypeError);
await assert.rejects(import('./dist/main-const.js'), ReferenceError);
await assert.rejects(import('./dist/main-cycle.js'), ReferenceError);
