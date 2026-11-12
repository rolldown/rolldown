import assert from 'node:assert/strict';
import { read } from './dist/main.js';

assert.deepEqual(read(), [true, true, false]);
