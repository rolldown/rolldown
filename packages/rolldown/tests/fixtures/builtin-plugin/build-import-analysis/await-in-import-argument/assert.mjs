// @ts-nocheck
import assert from 'node:assert';
import { load, loadThen } from './dist/main';

// the assertion shall pass with no syntax error
assert.strictEqual(typeof load, 'function');
assert.strictEqual(typeof loadThen, 'function');
