import assert from 'node:assert';
import './umd.js';
import { value } from './reader.js';

// `umd.js` has no module syntax and is both imported and required, so it is CommonJS
// and its top-level `this` is its exports object.
assert.strictEqual(value, 1);
