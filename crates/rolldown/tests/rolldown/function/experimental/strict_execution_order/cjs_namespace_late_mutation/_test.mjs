import assert from 'node:assert';

const { result } = await import('./dist/main.js');

assert.deepStrictEqual(result, {
  existing: 'existing',
  addedType: 'function',
  defaultAddedType: 'function',
});
