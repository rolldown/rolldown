// @ts-nocheck
import assert from 'node:assert';
import { a } from './dist/main';

// a should be the module object from lib2
assert.strictEqual(a.bar, 200);
