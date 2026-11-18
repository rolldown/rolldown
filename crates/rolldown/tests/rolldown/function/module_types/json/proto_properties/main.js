import assert from 'node:assert/strict';
import object from './object.json';
import array from './array.json';

const required = require('./required.json');

assert.deepStrictEqual(
  object,
  JSON.parse(
    '{"__proto__":0,"object":{"__proto__":{"marker":42}},"nil":{"__proto__":null},"array":[{"__proto__":"value"}]}',
  ),
);
assert.deepStrictEqual(array, JSON.parse('[{"__proto__":false},{"nested":{"__proto__":2}}]'));
assert.deepStrictEqual(
  required,
  JSON.parse('{"__proto__":{"marker":1},"nested":{"__proto__":null},"array":[{"__proto__":42}]}'),
);
