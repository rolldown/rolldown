import assert from 'node:assert/strict';
import { names, outer } from './dep.js';

export const e = 99;
console.log(names);
assert.deepStrictEqual(names, [1, 'e', 'e', 'e', 'e']);
assert.strictEqual(outer, undefined);
