import assert from 'node:assert';
import './e.js';
assert.deepStrictEqual(globalThis.log, ['p', 's', 'e']);
