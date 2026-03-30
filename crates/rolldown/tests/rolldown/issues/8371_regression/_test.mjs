import assert from 'node:assert';
import { t as require_cjs } from './dist/cjs.js';

// Regression test for #8371: verify that the fix for over-conservative
// circular dependency detection does not re-introduce the #8361 bug.
// The runtime chunk must NOT be merged into the entry chunk when it would
// create a circular static import chain.
assert.strictEqual(require_cjs(), 42);
