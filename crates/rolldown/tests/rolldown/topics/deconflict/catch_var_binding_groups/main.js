import assert from 'node:assert/strict';
import { run, iterate, nested } from './dep.js';
import { update } from './value.js';

update(99);

assert.deepStrictEqual(run(), [2, 99, 3, undefined]);
assert.deepStrictEqual(iterate(), [4, 5, 6, 7, 'key', 99, undefined]);
assert.deepStrictEqual(nested(), [1, 3, 99, 1, undefined]);
