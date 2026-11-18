import assert from 'node:assert/strict';
import { direct, onlyCatch, results } from './self.js';
import { cycle, result } from './cycle.js';
import { log } from './renamed.js';

assert.deepStrictEqual(results, [2, 3, undefined, 4, undefined]);
assert.strictEqual(direct, 2);
assert.strictEqual(onlyCatch, undefined);
assert.strictEqual(result, 11);
assert.strictEqual(cycle, 11);

let e = 'entry';
e += '!';
let f = 'entry';
f += '!';
assert.strictEqual(e, 'entry!');
assert.strictEqual(f, 'entry!');
let g = 0;
g++;
assert.strictEqual(g, 1);
assert.deepStrictEqual(log, ['v!', 'v!', 4, 'w!', 'w!', 6, 10, 9, 12]);
