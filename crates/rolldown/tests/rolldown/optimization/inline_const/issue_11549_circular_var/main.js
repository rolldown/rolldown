import assert from 'node:assert';
import { early, names } from './b.js';

export var v = 3;
var defaultValue = 4;
export { defaultValue as default };
export var __proto__ = 5;
export var same = 6;

console.log(early, v);
assert.deepStrictEqual([early, v], [undefined, 3]);
assert.deepStrictEqual(names, [undefined, undefined, undefined, 'local']);
