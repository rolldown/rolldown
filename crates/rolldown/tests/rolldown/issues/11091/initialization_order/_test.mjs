import assert from 'node:assert/strict';

const { Foo, bar } = await import('./dist/entry.js');

assert.deepEqual(Foo, { name: 'Foo', snapshot: 'before' });
assert.deepEqual(bar, { name: 'bar', snapshot: 'after' });
