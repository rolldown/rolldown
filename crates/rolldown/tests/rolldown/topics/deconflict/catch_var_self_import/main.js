import assert from 'node:assert/strict';
import { direct, onlyCatch, results } from './self.js';
import { cycle, result } from './cycle.js';

assert.deepStrictEqual(results, [2, 3, undefined, 4, undefined]);
assert.strictEqual(direct, 2);
assert.strictEqual(onlyCatch, undefined);
assert.strictEqual(result, 11);
assert.strictEqual(cycle, 11);
