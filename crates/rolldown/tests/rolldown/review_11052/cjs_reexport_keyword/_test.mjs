import assert from 'node:assert';
const m = await import('./dist/entry.js');
assert.deepStrictEqual({ ...m }, { for: 1, foo: 2 });
