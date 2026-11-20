import assert from 'node:assert';
import { log } from './dist/main.js';

assert.deepStrictEqual(log, ['a: Error: A failed', 'b: Error: A failed']);
