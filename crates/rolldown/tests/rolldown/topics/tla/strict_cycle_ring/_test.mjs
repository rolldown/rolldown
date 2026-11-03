import assert from 'node:assert';
import { value } from './dist/main.js';

assert.strictEqual(value, 'a');
assert.strictEqual(globalThis.__ringB, 'b');
