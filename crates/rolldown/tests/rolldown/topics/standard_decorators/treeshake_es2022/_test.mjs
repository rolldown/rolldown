import assert from 'node:assert';
import { value } from './dist/main.js';
assert.strictEqual(value, 3);
assert.strictEqual(globalThis.__decCalls, 3);
