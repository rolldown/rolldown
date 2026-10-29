import assert from 'node:assert';

const { targetPromise } = await import('./dist/a.js');
const target = await targetPromise;

assert.strictEqual(target.value, 1);
