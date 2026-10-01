import assert from 'node:assert/strict';

const { observed } = await import('./dist/main.js');
const expected = {
  indirect: 'modified',
  chain2: 'modified',
  chain3: 'modified',
};

assert.deepEqual(observed.fooIndirect, expected);
assert.deepEqual(observed.fooDirect, expected);
assert.deepEqual(observed.fooChained, expected);
assert.deepEqual(observed.fooPure, expected);
assert.deepEqual(observed.namespaceFoo, { namespace: 'modified' });
