import assert from 'node:assert/strict';
import value, { value as namedValue } from './dist/data.js';

assert.equal(globalThis.jsonAppendedExpressionRan, true);
assert.equal(namedValue, 1);
assert.deepEqual(value, { value: 1 });
