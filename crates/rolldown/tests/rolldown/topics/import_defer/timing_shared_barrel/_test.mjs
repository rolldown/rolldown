import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), [1, 1]);
