import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), ['r1', 'r2', 's', 'a', 'x', 'q', 'p', 'r1', 'r2']);
