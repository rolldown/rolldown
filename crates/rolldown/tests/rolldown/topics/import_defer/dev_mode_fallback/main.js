import assert from 'node:assert';
import { log } from './log.js';
import defer * as ns from './x.js';

assert.deepStrictEqual(log, ['x']);
assert.strictEqual(ns.value, 1);
export const loadY = () => import.defer('./y.js');
