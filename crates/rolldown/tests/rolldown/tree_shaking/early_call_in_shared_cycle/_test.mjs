import assert from 'node:assert/strict';

await assert.doesNotReject(import('./dist/main.js'));
