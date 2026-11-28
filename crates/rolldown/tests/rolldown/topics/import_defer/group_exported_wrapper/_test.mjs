import assert from 'node:assert';
import { dynamic, run } from './dist/main.js';

assert.strictEqual(run(), 'function');
const ns = await dynamic();
assert.deepStrictEqual(Object.keys(ns), ['f']);
assert.strictEqual(typeof ns.f, 'function');
