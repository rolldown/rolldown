import { bar as a } from './foo.js';
import assert from 'node:assert';

import('./foo.js').then(({ default: { bar: b } }) => {
  assert.strictEqual(b, 123);
  assert.strictEqual(a, 123);
});
