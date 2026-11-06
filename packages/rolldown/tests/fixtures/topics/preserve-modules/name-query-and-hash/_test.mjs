import assert from 'node:assert';
import values from './dist/main.mjs';

assert.deepStrictEqual(values, ['data', 'x', 'card']);
assert.deepStrictEqual(globalThis.nameQueryAndHash, ['data', 'x', 'card']);
