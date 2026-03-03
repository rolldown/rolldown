import assert from 'node:assert';
import { b } from './dist/a/index.js';

assert.strictEqual(b, 2, 'b should be 2');
