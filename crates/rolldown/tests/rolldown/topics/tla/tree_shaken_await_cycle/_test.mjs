import assert from 'node:assert/strict';
import { result } from './dist/main.js';

assert.equal(result, 'page');
assert.deepEqual(globalThis.tlaOrder, ['utils', 'fetch', 'page', 'respond']);
