import { shared } from './shared.js';
import assert from 'node:assert';

assert.strictEqual(shared, 'shared');

import('./dyn.js').then((m) => {
  assert.strictEqual(m.dynVal, 'dyn:shared');
});
