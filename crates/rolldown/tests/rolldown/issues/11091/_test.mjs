import assert from 'node:assert/strict';

const { Foo } = await import('./dist/entry.js');

assert.deepEqual(Foo, { name: 'Foo' });
