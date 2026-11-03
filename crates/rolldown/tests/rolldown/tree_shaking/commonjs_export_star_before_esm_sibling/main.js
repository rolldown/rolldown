import assert from 'node:assert';
import { esm } from './barrel.js';
assert.strictEqual(esm, 1);
assert.deepStrictEqual(globalThis.evaluated, ['dep', 'esm']);
