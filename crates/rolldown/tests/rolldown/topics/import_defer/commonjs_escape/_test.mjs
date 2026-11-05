import assert from 'node:assert';
import {
  after,
  before,
  dynamicRead,
  exported,
  foo,
  isObject,
  keys,
  local,
  tag,
  thenInO,
  wrappedBar,
  writeError,
} from './dist/main.js';

assert.strictEqual(isObject, true);
assert.strictEqual(tag, '[object Deferred Module]');
assert.strictEqual(thenInO, false);
assert.deepStrictEqual(before, []);
assert.strictEqual(foo, 'foo');
assert.deepStrictEqual(after, ['cjs']);
assert.strictEqual(dynamicRead, 'hidden');
assert.deepStrictEqual(keys, ['bar', 'default', 'foo', 'hidden']);
assert.strictEqual(writeError, 'TypeError');
assert.strictEqual(local, 'bar');
assert.strictEqual(exported.foo, 'foo');
assert.strictEqual(exported.then, undefined);
assert.strictEqual(wrappedBar, 'bar');
