import assert from 'node:assert/strict';
import { result } from './dist/main.js';

assert.deepEqual(result, { text: '31', sum: '31' });
