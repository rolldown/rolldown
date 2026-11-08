import { strict as assert } from 'node:assert';
import { f, otherJoin } from './dist/main.js';
assert.deepEqual(f('param'), ['local-join', 'param']);
assert.equal(otherJoin, 'other-join');
