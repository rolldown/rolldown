import { a as c } from './lib.js';
import assert from 'node:assert';

import('./lib.js').then((mod) => {
  assert.strictEqual(mod.a, 123);
  assert.strictEqual(c, 123);
});
