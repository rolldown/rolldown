import assert from 'node:assert';
import { t as require_cjs } from './dist/cjs.js';

// The bug was `__commonJSMin is not a function` due to circular static imports
// caused by chunk optimization merging the runtime into the entry chunk.
// Verify the CJS module loads correctly and __commonJSMin is callable.
assert.strictEqual(require_cjs(), 42);
