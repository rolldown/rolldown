import assert from 'node:assert';

globalThis.__events = [];
const { result } = await import('./dist/main.js');

assert.deepStrictEqual(result, {
  cloned: { value: 1 },
  cn: 'x',
  keys: ['cloneDeep', 'cn'],
});
assert.deepStrictEqual(globalThis.__events, ['cn', 'clone-deep']);
