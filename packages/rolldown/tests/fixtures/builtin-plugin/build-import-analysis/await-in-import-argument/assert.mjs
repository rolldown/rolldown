// @ts-nocheck
import assert from 'node:assert';
import { load, loadThen } from './dist/main';

// Importing at all is the assertion: a synchronous preload thunk makes this
// module a syntax error, so the import throws before these run.
assert.strictEqual(typeof load, 'function');
assert.strictEqual(typeof loadThen, 'function');
