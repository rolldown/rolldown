import assert from 'node:assert/strict';
import m1 from './manual1.js';
import m2 from './manual2.js';

assert.deepEqual(m1, ['manual1', 'shared']);
assert.deepEqual(m2, ['manual2', 'shared']);
