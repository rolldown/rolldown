import assert from 'node:assert';
import { log, read } from './dist/main.js';

// `m0.js` to `m6.js` form one cycle, and `m3.js` has top-level await. `m2.js` and `m5.js` import
// through `export *`. As in native ESM, each module runs after its imports finish, except an import
// of a module that is still running (an ancestor). The `export *` imports must pass the importer to
// the state record, else the bundle runs `m6.js` before `m1.js` finishes, or waits forever.
assert.deepStrictEqual(log, ['0', '2', '1', '6', '5', '4', '3']);
assert.deepStrictEqual(read(), [0, 2]);
