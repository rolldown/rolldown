import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), [1, 2, ['m1', 'c1', 'p1', 'c2', 'p2', 'q2']]);
