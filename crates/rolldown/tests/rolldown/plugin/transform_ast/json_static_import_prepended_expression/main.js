import assert from 'node:assert/strict';
import value, { old } from './data.json';

assert.equal(old, 1);
assert.deepEqual(value, { old: 1 });
