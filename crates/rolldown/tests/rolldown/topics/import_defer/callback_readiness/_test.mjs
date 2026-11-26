import assert from 'node:assert/strict';
import { again, depValue, targetValue } from './dist/main.js';

assert.deepEqual(globalThis.events, ['TypeError', 'independent', 'target', 'TypeError']);
assert.deepEqual([depValue, targetValue, again], [2, 1, 1]);
