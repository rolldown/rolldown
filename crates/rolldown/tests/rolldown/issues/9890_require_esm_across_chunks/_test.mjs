import assert from 'node:assert/strict';

const { default: first } = await import('./dist/first.js');
const { default: second } = await import('./dist/second.js');
assert.strictEqual(first.namespace, second.namespace);
