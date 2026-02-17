import assert from 'node:assert';
import json from './dist/main.js';

assert.deepEqual(json.foo, '__EXP__', 'JSON import should match expected value');
