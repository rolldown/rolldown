import assert from 'node:assert/strict';
import * as e1 from './dist/e1.js';
import * as e2 from './dist/e2.js';

for (const entry of [e1, e2]) {
  assert.deepEqual(Object.keys(entry), ['default', 'x']);
  assert.deepEqual(entry.default, { x: 1 });
  assert.equal(entry.x, 1);
}
