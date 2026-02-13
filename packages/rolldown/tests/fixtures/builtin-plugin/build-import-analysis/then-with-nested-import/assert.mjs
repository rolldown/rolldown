// @ts-nocheck
import assert from 'node:assert';
import { a } from './dist/main';

// a should be the bar value from lib2
assert.strictEqual(a, 200);
