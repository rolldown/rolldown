import assert from 'node:assert/strict';

await assert.rejects(import('./dist/main.js'), { message: 'before' });
