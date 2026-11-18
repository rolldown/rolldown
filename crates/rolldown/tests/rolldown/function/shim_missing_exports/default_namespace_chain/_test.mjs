import assert from 'node:assert/strict';
import { keys, value } from './dist/main.js';

assert.equal(value, undefined);
assert.deepEqual(keys, ['missing', 'value']);
