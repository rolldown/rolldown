import assert from 'node:assert';
import { ready, value } from './dist/main.js';

assert.strictEqual(value, 42);
assert.strictEqual(ready, true);
