import { read } from './indirect';
import './read';
import assert from 'node:assert';

assert.strictEqual(typeof read, 'function');

import('./indirect').then((mod) => {
  assert.strictEqual(typeof mod.read, 'function');
  console.log(mod);
});
