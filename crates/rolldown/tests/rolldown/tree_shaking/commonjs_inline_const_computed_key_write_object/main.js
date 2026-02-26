import assert from 'node:assert';
import * as ns from './src/cjs.js';
import cjs from './src/cjs.js';

const obj = {};
// ns.a and cjs.a are reads used as computed keys — they should still be inlined.
obj[ns.a] = 1;
obj[cjs.a] = 2;

assert.equal(ns.a, 'cjs-a');
assert.equal(cjs.a, 'cjs-a');
assert.equal(obj['cjs-a'], 2);
