import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { y } from './shared.js';

assert.strictEqual(typeof y, 'function');
assert.strictEqual(y, readFileSync);
