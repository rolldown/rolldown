import assert from 'node:assert/strict';
import { result } from './dist/main.js';

assert.deepEqual(result, ['x', 'a', 'b']);
