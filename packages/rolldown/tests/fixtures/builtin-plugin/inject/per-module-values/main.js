import assert from 'node:assert';
import { depFoo } from './vendor/dep.js';

assert.strictEqual(foo(), 'app');
assert.strictEqual(depFoo(), 'shim');
