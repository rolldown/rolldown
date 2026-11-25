import assert from 'node:assert/strict';
import { result } from './dist/later.js';

assert.deepEqual(result, ['ready', 'lib']);
