import assert from 'node:assert/strict';
import { keys, values, load } from './dist/main.js';

assert.deepEqual(values, [undefined, undefined, undefined]);
assert.deepEqual(keys, [
  ['a-b', 'default', 'missing', 'unused', 'value'],
  ['a-b', 'missing', 'unused', 'value'],
]);
const namespace = await load();
for (const name of keys[1]) {
  assert.ok(Object.hasOwn(namespace, name));
  assert.equal(namespace[name], name === 'value' ? 1 : undefined);
}
assert.equal(namespace.value, 1);
