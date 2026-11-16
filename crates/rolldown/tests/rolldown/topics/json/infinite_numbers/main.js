import assert from 'node:assert/strict';
import value from './data.json';

const Infinity = Date.now();
assert.equal(typeof Infinity, 'number');
assert.equal(value.pos, Number.POSITIVE_INFINITY);
assert.equal(value.neg, Number.NEGATIVE_INFINITY);
