import assert from 'node:assert';
import { value } from './lib/P.js';
import { Z } from './lib/M.js';

assert.strictEqual(value(), 1);
assert.strictEqual(Z, 'z');
