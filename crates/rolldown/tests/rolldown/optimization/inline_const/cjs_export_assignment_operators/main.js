import assert from 'node:assert';
import {
  add,
  subtract,
  multiply,
  divide,
  remainder,
  exponent,
  shiftLeft,
  shiftRight,
  unsignedShiftRight,
  bitAnd,
  bitOr,
  bitXor,
  logicalAnd,
  logicalOr,
  nullish,
  constant,
} from './dep.cjs';

assert.strictEqual(add ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(logicalAnd ? 'truthy' : 'falsy', 'falsy');
assert.strictEqual(constant ? 'truthy' : 'falsy', 'truthy');

assert.strictEqual(add, NaN);
assert.strictEqual(subtract, NaN);
assert.strictEqual(multiply, NaN);
assert.strictEqual(divide, NaN);
assert.strictEqual(remainder, NaN);
assert.strictEqual(exponent, NaN);
assert.strictEqual(shiftLeft, 0);
assert.strictEqual(shiftRight, 0);
assert.strictEqual(unsignedShiftRight, 0);
assert.strictEqual(bitAnd, 0);
assert.strictEqual(bitOr, 1);
assert.strictEqual(bitXor, 1);
assert.strictEqual(logicalAnd, undefined);
assert.strictEqual(logicalOr, 2);
assert.strictEqual(nullish, 3);
assert.strictEqual(constant, true);
