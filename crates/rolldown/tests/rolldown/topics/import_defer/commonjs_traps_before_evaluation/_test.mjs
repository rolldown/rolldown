import assert from 'node:assert';
import {
  after,
  before,
  defineSymbol,
  defineThen,
  defineThenAfter,
  keys,
  otherTag,
  protoIsNull,
  sameTag,
  setProto,
  setProtoAfter,
  setProtoNull,
  symbolInO,
  thenType,
} from './dist/main.js';

// Before the first evaluation. Native Node.js 26.8.1 gives the same results.
assert.strictEqual(defineThen, false);
assert.strictEqual(defineSymbol, false);
assert.strictEqual(symbolInO, false);
assert.strictEqual(thenType, 'undefined');
assert.strictEqual(sameTag, true);
assert.strictEqual(otherTag, false);
assert.strictEqual(setProto, false);
assert.strictEqual(setProtoNull, true);
assert.strictEqual(protoIsNull, true);
assert.deepStrictEqual(before, []);
// After the first evaluation.
assert.deepStrictEqual(keys, ['default', 'foo']);
assert.deepStrictEqual(after, ['cjs']);
assert.strictEqual(defineThenAfter, false);
assert.strictEqual(setProtoAfter, false);
