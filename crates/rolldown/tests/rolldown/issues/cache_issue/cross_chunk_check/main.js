import { fromA } from './a.js';
import { fromB } from './b.js';
import assert from 'node:assert';

assert.strictEqual(fromA, fromB);
assert.strictEqual(fromA.value, 1);
assert.strictEqual(fromB.value, 1);
