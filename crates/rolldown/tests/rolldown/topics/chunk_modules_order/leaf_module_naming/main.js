import assert from 'node:assert';
import { a } from './a.js';
import { b } from './b.js';

assert.deepStrictEqual([a(), b()], ['a', 'b']);
