import assert from 'node:assert/strict';

import { namespace, namespaceKeys } from './dist/index.js';

const dataModule = await import('./dist/data.js');
const expectedDefault = {
  answer: 42,
  nested: {
    ok: true,
  },
};
const expectedKeys = ['answer', 'default', 'nested'];

assert.deepEqual(namespaceKeys, expectedKeys);
assert.deepEqual(Object.keys(namespace).sort(), expectedKeys);
assert.equal(namespace.answer, 42);
assert.deepEqual(namespace.nested, { ok: true });
assert.deepEqual(namespace.default, expectedDefault);

console.log(`data.dataModule: `, dataModule);
assert.equal(dataModule.answer, 42);
assert.deepEqual(dataModule.nested, { ok: true });
assert.deepEqual(dataModule.default, expectedDefault);
