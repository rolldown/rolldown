import assert from 'node:assert';
import { bar as a } from './foo.js';

import('./foo.js').then(({ bar: b }) => {
  assert.strictEqual(b, 123);
  assert.strictEqual(a, 123);
});
