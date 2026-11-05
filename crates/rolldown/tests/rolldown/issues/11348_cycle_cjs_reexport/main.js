import assert from 'node:assert';
import a from './a.cjs';

assert.strictEqual(a.value, 123);
