import { strict as assert } from 'node:assert';
import { read } from './dist/main.js';

// Pre-fix, `__toESM(...)` called the parameter: `TypeError: __toESM is not a function`.
assert.deepEqual(await read('param'), [1, 'param']);
