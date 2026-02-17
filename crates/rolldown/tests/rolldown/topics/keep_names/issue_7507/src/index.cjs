import assert from 'node:assert';
import { test as barTest } from './bar.cjs';
import test from './foo.cjs';

assert.strictEqual(barTest.name, 'test');
assert.strictEqual(test.name, 'test');
