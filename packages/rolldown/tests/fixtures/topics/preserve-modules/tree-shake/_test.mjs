import assert from 'node:assert';
import lib from './dist/main.js';

assert.strictEqual(lib.lib, 'lib');
assert.strictEqual(lib.a, undefined);
