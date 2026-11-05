import assert from 'node:assert';
import { log, result } from './dist/main.js';

assert.deepStrictEqual(await result, ['d failed', true]);
assert.deepStrictEqual(log, ['d']);
