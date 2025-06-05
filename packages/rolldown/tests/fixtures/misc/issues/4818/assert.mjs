import json from './dist/main'
import assert from 'assert'

assert.deepEqual(json.foo, '__EXP__', 'JSON import should match expected value');

