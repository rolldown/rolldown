import assert from 'node:assert';
// #6010: the named export must survive on the individual preserved module file.
// Without the fix `dist/three.js` omits `myVariable`, so this import would be
// `undefined` (and fail at ESM link time in stricter loaders).
import { myVariable } from './dist/three.js';
// It must also stay reachable through the `export * as` namespace chain.
import { two } from './dist/index.js';

assert.strictEqual(myVariable, 'world');
assert.strictEqual(two.three.myVariable, 'world');
