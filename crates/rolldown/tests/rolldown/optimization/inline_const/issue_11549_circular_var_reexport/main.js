import assert from 'node:assert';
import { early } from './b.js';

export var v = 3;
var defaultValue = 4;
export { defaultValue as default };
export var __proto__ = 5;

assert.deepStrictEqual(early, [undefined, undefined, undefined]);
assert.deepStrictEqual([v, defaultValue, __proto__], [3, 4, 5]);
