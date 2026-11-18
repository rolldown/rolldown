import assert from 'node:assert';
import './barrel.js';

assert.deepStrictEqual(globalThis.evaluated, ['dep']);
