import assert from 'node:assert';
import { log, result } from './dist/main.js';

assert.deepStrictEqual(result, [true, true, true, true]);
assert.deepStrictEqual(log, ['dep']);
