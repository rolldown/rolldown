import assert from 'node:assert';
import './c1.js';
import './c2.js';
assert.deepStrictEqual(globalThis.log, ['a', 'b', 'c1', 'c2']);
