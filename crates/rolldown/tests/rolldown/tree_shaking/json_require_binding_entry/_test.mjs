import assert from 'node:assert/strict';
import data from './dist/data.js';

data.foo = 'mutated';
const { value } = await import('./dist/reader.js');
assert.strictEqual(value, 'mutated');
