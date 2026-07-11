import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { x } from './shared.js';

assert.strictEqual(typeof x, 'function');
assert.strictEqual(x, readFileSync);
