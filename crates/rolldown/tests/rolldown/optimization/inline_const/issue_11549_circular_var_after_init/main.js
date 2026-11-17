import assert from 'node:assert';
import { read } from './b.js';

export var v = 3;

console.log(read(), v);
assert.deepStrictEqual([read(), v], [3, 3]);
