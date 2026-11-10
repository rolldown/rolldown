import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), [true, false]);
