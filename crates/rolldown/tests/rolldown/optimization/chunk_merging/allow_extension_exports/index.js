import { value1 } from './lib1.js';
import { value1 as v1, value2 } from './lib2.js';

import assert from 'node:assert';

assert.strictEqual(value1, 'lib1-value');
assert.strictEqual(v1, 'conflict-value');
assert.strictEqual(value2, 'lib2-value');

import('./lib3.js').then(m => {
  assert.strictEqual(m.value3, 'lib3-value');
  assert.strictEqual(m.value4, 'lib3-value4');
});
