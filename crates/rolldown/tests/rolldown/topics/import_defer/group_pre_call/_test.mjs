import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), [1, 2, 'shared', 'member', 'first', 'second']);
