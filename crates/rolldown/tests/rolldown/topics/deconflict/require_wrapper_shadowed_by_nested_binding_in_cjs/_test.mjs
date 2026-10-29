import { strict as assert } from 'node:assert';
import { read } from './dist/main.js';

// Pre-fix, `require_dep()` called the parameter: `TypeError: require_dep is not a function`.
assert.deepEqual(read('param'), [1, 'param']);
