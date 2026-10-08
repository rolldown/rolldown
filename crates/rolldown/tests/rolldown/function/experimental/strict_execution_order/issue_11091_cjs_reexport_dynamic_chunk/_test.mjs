import assert from 'node:assert';

const { Foo } = await import('./dist/entry.js');

assert.deepStrictEqual(Foo, { name: 'Foo' });
