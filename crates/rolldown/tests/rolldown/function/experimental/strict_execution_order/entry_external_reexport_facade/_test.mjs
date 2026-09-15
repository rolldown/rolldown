import assert from 'node:assert';

const namespace = await import('./dist/main.js');

assert.strictEqual(namespace.externalValue, 42);
