import assert from 'node:assert';
import { before, log, values } from './dist/main.js';

// `main.js` requires the file of `a.js` before its body runs, but that load does not run `a.js`.
assert.deepStrictEqual(before, ['main']);
// The first access runs `a.js`. Each read then gets the value that `init_a()` assigned.
assert.deepStrictEqual(values, ['a', 1, 'function', 'a']);
assert.deepStrictEqual(log, ['main', 'a']);
